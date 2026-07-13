// One isolated Bun Worker per loaded Termy plugin.
import { createHash } from "node:crypto";
import {
  mkdir,
  readFile,
  readdir,
  rename,
  rm,
  writeFile,
} from "node:fs/promises";
import { isBuiltin } from "node:module";
import {
  dirname,
  isAbsolute,
  join,
  relative,
  resolve,
  sep,
} from "node:path";
import { pathToFileURL } from "node:url";

type PluginSource = {
  id: string;
  root: string;
  cacheKey: string;
};
type PreparedPluginSource = PluginSource & {
  name: string;
  version?: string;
  path: string;
};
type PluginManifest = {
  apiVersion: number;
  id: string;
  name: string;
  version?: string;
  main?: string;
};
type CapturedFile = { relativePath: string; contents: Uint8Array };
type PluginToastLevel = "info" | "success" | "warning" | "error";
type PluginToasts = {
  info: (message: string) => void;
  success: (message: string) => void;
  warning: (message: string) => void;
  error: (message: string) => void;
};
type PluginJsonValue =
  | null
  | boolean
  | number
  | string
  | PluginJsonValue[]
  | { [key: string]: PluginJsonValue };
type PluginStorage = {
  get: <T = PluginJsonValue>(key: string) => Promise<T | undefined>;
  set: (key: string, value: PluginJsonValue) => Promise<void>;
  delete: (key: string) => Promise<boolean>;
  clear: () => Promise<void>;
};
type PluginPaths = {
  dataDirectory: string;
  cacheDirectory: string;
};
type PluginSettings = {
  get: <T = string | boolean>(key: string) => T | undefined;
};
type PluginServices = { storage: PluginStorage; paths: PluginPaths };
type PluginContext = Record<string, unknown> &
  PluginServices & { settings: PluginSettings; toasts: PluginToasts };
type PluginCommand = {
  id: string;
  title: string;
  keywords?: string[];
  status?: string;
  enabled?: boolean;
  disabledReason?: string;
  icon?: string;
  inputs?: unknown[];
  timeoutMs?: number;
  run: (request: {
    inputs: Record<string, unknown>;
    context: PluginContext;
  }) => unknown;
};
type PluginSettingDefinition = {
  type: "toggle" | "text" | "select" | "secret";
  title: string;
  description?: string;
  placeholder?: string;
  defaultValue?: string | boolean;
  maxLength?: number;
  options?: Array<{ value: string; label: string }>;
};
type PluginDefinition = {
  commands: PluginCommand[];
  settings?: Record<string, PluginSettingDefinition>;
};

declare global {
  var definePlugin: <T extends PluginDefinition>(plugin: T) => T;
}

const MAX_PLUGIN_TREE_BYTES = 16 * 1024 * 1024;
const MAX_PLUGIN_TREE_FILES = 4_096;
const MAX_STORAGE_BYTES = 1024 * 1024;
const MAX_STORAGE_ENTRIES = 512;
globalThis.definePlugin = (plugin) => plugin;
let plugin: PluginDefinition | undefined;
let pluginServices: PluginServices | undefined;
let commandHandlers = new Map<string, PluginCommand["run"]>();
let queue = Promise.resolve();

// Keep ordinary plugin output on Termy's diagnostic stream.
process.stdout.write = process.stderr.write.bind(process.stderr) as typeof process.stdout.write;

function errorMessage(error: unknown): string {
  const message = error instanceof Error ? error.message : String(error);
  if (
    error &&
    typeof error === "object" &&
    Array.isArray((error as { logs?: unknown }).logs)
  ) {
    const detail = (error as { logs: Array<{ message?: unknown }> }).logs
      .map((log) => String(log.message || log))
      .join("; ");
    if (detail) return `${message}: ${detail}`;
  }
  if (error instanceof Error && error.stack && message === "Bundle failed") {
    return error.stack;
  }
  return message;
}

function logValue(value: unknown): string {
  if (typeof value === "string") return value;
  try {
    return JSON.stringify(value);
  } catch {
    return String(value);
  }
}

for (const level of ["log", "info", "warn", "error"] as const) {
  console[level] = (...values: unknown[]) => {
    const pluginId = process.env.TERMY_PLUGIN_ID || "unknown";
    process.stderr.write(
      `[termy plugin ${pluginId}] ${values.map(logValue).join(" ")}\n`,
    );
  };
}

function assertId(value: unknown, label: string): asserts value is string {
  if (
    typeof value !== "string" ||
    !/^[a-z0-9][a-z0-9._-]{0,63}$/.test(value)
  ) {
    throw new Error(`${label} must be a lowercase stable ID`);
  }
}

function assertText(value: unknown, label: string, max = 300): asserts value is string {
  if (typeof value !== "string" || value.trim() === "" || value.length > max) {
    throw new Error(`${label} must be a non-empty string up to ${max} characters`);
  }
}

function optionalText(value: unknown, label: string, max: number): string | undefined {
  if (value === undefined) return undefined;
  assertText(value, label, max);
  return value;
}

function optionalString(value: unknown, label: string, max: number): string | undefined {
  if (value === undefined) return undefined;
  if (typeof value !== "string" || value.length > max) {
    throw new Error(`${label} must be a string up to ${max} characters`);
  }
  return value;
}

function normalizeKeywords(value: unknown, label: string): string[] {
  if (value === undefined) return [];
  if (!Array.isArray(value) || value.length > 64) {
    throw new Error(`${label} must contain at most 64 strings`);
  }
  return value.map((keyword) => {
    assertText(keyword, label, 200);
    return keyword;
  });
}

async function capturePlugin(source: PluginSource): Promise<CapturedFile[]> {
  const root = resolve(source.root);
  const files: CapturedFile[] = [];
  let totalBytes = 0;

  const visit = async (directory: string, prefix: string): Promise<void> => {
    const entries = await readdir(directory, { withFileTypes: true });
    entries.sort((left, right) =>
      Buffer.compare(Buffer.from(left.name), Buffer.from(right.name)),
    );
    for (const entry of entries) {
      if (
        entry.name === ".git" ||
        entry.name === "node_modules" ||
        entry.name === ".termy-disabled" ||
        entry.name === ".termy-source.json"
      ) {
        continue;
      }
      const relativePath = prefix ? `${prefix}/${entry.name}` : entry.name;
      const path = join(directory, entry.name);
      if (entry.isSymbolicLink()) {
        throw new Error(`Plugin source cannot contain symlink ${relativePath}`);
      }
      if (entry.isDirectory()) {
        await visit(path, relativePath);
        continue;
      }
      if (!entry.isFile()) {
        throw new Error(`Plugin source contains unsupported file ${relativePath}`);
      }
      const contents = new Uint8Array(await readFile(path));
      totalBytes += contents.byteLength;
      if (totalBytes > MAX_PLUGIN_TREE_BYTES) {
        throw new Error("Plugin source tree exceeds 16 MiB");
      }
      files.push({ relativePath, contents });
      if (files.length > MAX_PLUGIN_TREE_FILES) {
        throw new Error(`Plugin source tree exceeds ${MAX_PLUGIN_TREE_FILES} files`);
      }
    }
  };

  await visit(root, "");
  files.sort((left, right) =>
    Buffer.compare(
      Buffer.from(left.relativePath),
      Buffer.from(right.relativePath),
    ),
  );
  const hash = createHash("sha256");
  hash.update("termy-plugin-bundle-v1\0");
  for (const file of files) {
    hash.update(file.relativePath);
    hash.update(new Uint8Array([0]));
    hash.update(file.contents);
    hash.update(new Uint8Array([0]));
  }
  const actualCacheKey = hash.digest("hex");
  if (actualCacheKey !== source.cacheKey) {
    throw new Error("Plugin changed while it was loading; reopen the command palette");
  }
  return files;
}

function parseManifest(source: PluginSource, files: CapturedFile[]): {
  manifest: PluginManifest;
  entrypoint: string;
} {
  const manifestFile = files.find((file) => file.relativePath === "plugin.json");
  if (!manifestFile) throw new Error("plugin.json is missing");
  let manifest: PluginManifest;
  try {
    manifest = JSON.parse(Buffer.from(manifestFile.contents).toString("utf8")) as PluginManifest;
  } catch (error) {
    throw new Error(`Invalid plugin.json: ${errorMessage(error)}`);
  }
  if (!manifest || typeof manifest !== "object") {
    throw new Error("plugin.json must be an object");
  }
  if (manifest.apiVersion !== 1) throw new Error("plugin.json apiVersion must be 1");
  assertId(manifest.id, "plugin.json id");
  if (manifest.id !== source.id) {
    throw new Error(`plugin.json id ${manifest.id} must match directory ${source.id}`);
  }
  assertText(manifest.name, "plugin.json name", 200);
  if (manifest.version !== undefined) {
    assertText(manifest.version, "plugin.json version", 100);
  }
  const main = manifest.main === undefined
    ? "plugin.ts"
    : optionalText(manifest.main, "plugin.json main", 1_024)!;
  if (isAbsolute(main)) throw new Error("plugin.json main must be inside the plugin directory");
  const root = resolve(source.root);
  const absoluteEntrypoint = resolve(root, main);
  const relativeEntrypoint = relative(root, absoluteEntrypoint);
  if (
    relativeEntrypoint === "" ||
    relativeEntrypoint === ".." ||
    relativeEntrypoint.startsWith(`..${sep}`) ||
    isAbsolute(relativeEntrypoint)
  ) {
    throw new Error("plugin.json main must be a file inside the plugin directory");
  }
  const entrypoint = relativeEntrypoint.split(sep).join("/");
  if (!files.some((file) => file.relativePath === entrypoint)) {
    throw new Error(`Plugin entrypoint does not exist: ${main}`);
  }
  return { manifest, entrypoint };
}

function pathIsInside(root: string, candidate: string): boolean {
  const relativePath = relative(root, candidate);
  return (
    relativePath !== "" &&
    relativePath !== ".." &&
    !relativePath.startsWith(`..${sep}`) &&
    !isAbsolute(relativePath)
  );
}

async function bundlePlugin(
  source: PluginSource,
  files: CapturedFile[],
  entrypoint: string,
  bundleCacheRoot: string,
): Promise<string> {
  const bundleDirectory = join(bundleCacheRoot, source.id);
  const bundlePath = join(bundleDirectory, `${source.cacheKey}.mjs`);
  await mkdir(bundleDirectory, { recursive: true });
  if (!(await Bun.file(bundlePath).exists())) {
    const snapshotRoot = join(
      bundleDirectory,
      `.capture-${process.pid}-${Date.now()}-${Math.random().toString(16).slice(2)}`,
    );
    try {
      for (const file of files) {
        const target = join(snapshotRoot, ...file.relativePath.split("/"));
        await mkdir(dirname(target), { recursive: true });
        await Bun.write(target, file.contents);
      }
      const snapshotEntrypoint = join(snapshotRoot, ...entrypoint.split("/"));
      let build: Awaited<ReturnType<typeof Bun.build>>;
      try {
        build = await Bun.build({
          entrypoints: [snapshotEntrypoint],
          target: "bun",
          format: "esm",
          minify: false,
          sourcemap: "inline",
          plugins: [
            {
              name: "termy-plugin-import-boundary",
              setup(builder) {
                builder.onResolve({ filter: /.*/ }, (args) => {
                  if (args.kind === "entry-point") return;
                  const specifier = args.path;
                  if (
                    isAbsolute(specifier) &&
                    args.importer === "" &&
                    pathIsInside(snapshotRoot, specifier)
                  ) {
                    return;
                  }
                  if (
                    isBuiltin(specifier) ||
                    specifier === "bun" ||
                    specifier.startsWith("bun:")
                  ) {
                    return;
                  }
                  if (specifier.startsWith(".")) {
                    const candidate = resolve(args.resolveDir, specifier);
                    if (pathIsInside(snapshotRoot, candidate)) return;
                    throw new Error(`Plugin import escapes its directory: ${specifier}`);
                  }
                  throw new Error(
                    `Plugin package or absolute import is not supported: ${specifier}`,
                  );
                });
              },
            },
          ],
        });
      } catch (error) {
        throw new Error(`Failed to bundle plugin ${source.id}: ${Bun.inspect(error)}`);
      }
      if (!build.success) {
        const detail = build.logs
          .map((log) => ("message" in log ? String(log.message) : String(log)))
          .join("; ");
        throw new Error(
          `Failed to bundle plugin ${source.id}: ${detail || "unknown error"}`,
        );
      }
      const output = build.outputs.find((artifact) => artifact.kind === "entry-point");
      if (!output) throw new Error(`Plugin ${source.id} produced no executable bundle`);
      const temporaryPath = `${bundlePath}.${process.pid}.${Date.now()}.tmp`;
      await Bun.write(temporaryPath, output);
      try {
        await rename(temporaryPath, bundlePath);
      } catch (error) {
        await rm(temporaryPath, { force: true });
        if (!(await Bun.file(bundlePath).exists())) throw error;
      }
    } finally {
      await rm(snapshotRoot, { recursive: true, force: true });
    }
  }
  for (const entry of await readdir(bundleDirectory)) {
    const path = join(bundleDirectory, entry);
    if (path !== bundlePath && entry.endsWith(".mjs")) {
      await rm(path, { force: true });
    }
  }
  return bundlePath;
}

async function preparePlugin(
  source: PluginSource,
  bundleCacheRoot: string,
): Promise<PreparedPluginSource> {
  assertId(source.id, "Plugin path ID");
  assertText(source.cacheKey, "Plugin cache key", 128);
  const files = await capturePlugin(source);
  const { manifest, entrypoint } = parseManifest(source, files);
  const path = await bundlePlugin(source, files, entrypoint, bundleCacheRoot);
  return {
    ...source,
    name: manifest.name,
    version: manifest.version,
    path,
  };
}

function normalizeInput(input: unknown, commandId: string, seen: Set<string>): unknown {
  if (!input || typeof input !== "object") {
    throw new Error(`Command ${commandId} has an invalid input`);
  }
  const value = input as Record<string, unknown>;
  assertId(value.id, `Input ID for ${commandId}`);
  if (seen.has(value.id)) throw new Error(`Duplicate input ID ${value.id}`);
  seen.add(value.id);
  assertText(value.label, `Input label for ${commandId}`, 200);
  if (value.type === "text") {
    if (
      value.maxLength !== undefined &&
      (!Number.isInteger(value.maxLength) || Number(value.maxLength) < 1 || Number(value.maxLength) > 16_384)
    ) {
      throw new Error(`Text input ${value.id} has an invalid maxLength`);
    }
    const maxLength = value.maxLength === undefined ? 1_024 : Number(value.maxLength);
    const defaultValue = optionalString(
      value.defaultValue,
      `Default value for ${value.id}`,
      maxLength,
    );
    return {
      type: "text",
      id: value.id,
      label: value.label,
      placeholder: optionalText(value.placeholder, `Placeholder for ${value.id}`, 300),
      defaultValue,
      required: value.required === true,
      maxLength,
    };
  }
  if (value.type === "select") {
    if (!Array.isArray(value.options) || value.options.length === 0 || value.options.length > 128) {
      throw new Error(`Select input ${value.id} must have 1 to 128 options`);
    }
    const optionValues = new Set<string>();
    const options = value.options.map((raw) => {
      if (!raw || typeof raw !== "object") throw new Error(`Invalid option in ${value.id}`);
      const option = raw as Record<string, unknown>;
      assertText(option.value, `Option value for ${value.id}`, 1_024);
      assertText(option.label, `Option label for ${value.id}`, 200);
      if (optionValues.has(option.value)) throw new Error(`Duplicate option ${option.value}`);
      optionValues.add(option.value);
      return {
        value: option.value,
        label: option.label,
        keywords: normalizeKeywords(option.keywords, `Option keywords for ${value.id}`),
        status: optionalText(option.status, `Option status for ${value.id}`, 200),
      };
    });
    const defaultValue = optionalText(value.defaultValue, `Default value for ${value.id}`, 1_024);
    if (defaultValue !== undefined && !optionValues.has(defaultValue)) {
      throw new Error(`Select input ${value.id} defaultValue must match an option`);
    }
    return {
      type: "select",
      id: value.id,
      label: value.label,
      placeholder: optionalText(value.placeholder, `Placeholder for ${value.id}`, 300),
      defaultValue,
      required: value.required !== false,
      options,
    };
  }
  if (value.type === "confirm") {
    if (value.defaultValue !== undefined && typeof value.defaultValue !== "boolean") {
      throw new Error(`Confirm input ${value.id} defaultValue must be a boolean`);
    }
    return {
      type: "confirm",
      id: value.id,
      label: value.label,
      defaultValue: value.defaultValue !== false,
    };
  }
  throw new Error(`Input ${value.id} has unsupported type ${String(value.type)}`);
}

function normalizePlugin(
  candidate: unknown,
  source: PreparedPluginSource,
): { commands: Record<string, unknown>[]; settings: Record<string, unknown>[] } {
  if (!candidate || typeof candidate !== "object") {
    throw new Error("Default export must be definePlugin({...})");
  }
  const definition = candidate as PluginDefinition;
  if (!Array.isArray(definition.commands) || definition.commands.length > 256) {
    throw new Error("Plugin commands must be an array with at most 256 entries");
  }
  const ids = new Set<string>();
  commandHandlers = new Map();
  const commands = definition.commands.map((command) => {
    if (!command || typeof command !== "object") throw new Error("Invalid plugin command");
    assertId(command.id, "Command ID");
    if (ids.has(command.id)) throw new Error(`Duplicate command ID ${command.id}`);
    ids.add(command.id);
    assertText(command.title, `Command title for ${command.id}`);
    if (typeof command.run !== "function") {
      throw new Error(`Command ${command.id} must define run()`);
    }
    if (command.enabled !== undefined && typeof command.enabled !== "boolean") {
      throw new Error(`Command ${command.id} enabled must be a boolean`);
    }
    const inputIds = new Set<string>();
    const inputs = Array.isArray(command.inputs)
      ? command.inputs.map((input) => normalizeInput(input, command.id, inputIds))
      : [];
    if (inputs.length > 16) throw new Error(`Command ${command.id} has too many inputs`);
    commandHandlers.set(command.id, command.run);
    if (
      command.timeoutMs !== undefined &&
      (!Number.isInteger(command.timeoutMs) || command.timeoutMs < 100 || command.timeoutMs > 30_000)
    ) {
      throw new Error(`Command ${command.id} timeoutMs must be between 100 and 30000`);
    }
    const timeoutMs = command.timeoutMs ?? 10_000;
    const icons = [
      "command",
      "play",
      "terminal",
      "folder",
      "link",
      "clipboard",
      "settings",
      "info",
    ];
    if (command.icon !== undefined && !icons.includes(command.icon)) {
      throw new Error(`Command ${command.id} has an unsupported icon`);
    }
    const disabledReason =
      command.enabled === false
        ? optionalText(
            command.disabledReason,
            `Disabled reason for ${command.id}`,
            300,
          ) || "Disabled by plugin"
        : undefined;
    return {
      pluginId: source.id,
      pluginName: source.name,
      id: command.id,
      title: command.title,
      keywords: normalizeKeywords(command.keywords, `Keywords for ${command.id}`),
      status: optionalText(command.status, `Status for ${command.id}`, 200),
      disabledReason,
      icon: command.icon || "command",
      inputs,
      timeoutMs,
    };
  });
  const settings = normalizeSettings(definition.settings);
  plugin = definition;
  return { commands, settings };
}

function normalizeSettings(value: unknown): Record<string, unknown>[] {
  if (value === undefined) return [];
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error("Plugin settings must be an object");
  }
  const entries = Object.entries(value as Record<string, unknown>);
  if (entries.length > 64) throw new Error("Plugin settings must have at most 64 entries");
  return entries.map(([id, candidate]) => {
    assertId(id, "Setting ID");
    if (!candidate || typeof candidate !== "object" || Array.isArray(candidate)) {
      throw new Error(`Setting ${id} must be an object`);
    }
    const setting = candidate as PluginSettingDefinition;
    assertText(setting.title, `Setting title for ${id}`, 200);
    const description = optionalText(
      setting.description,
      `Setting description for ${id}`,
      500,
    );
    if (setting.type === "toggle") {
      if (setting.defaultValue !== undefined && typeof setting.defaultValue !== "boolean") {
        throw new Error(`Setting ${id} defaultValue must be a boolean`);
      }
      return {
        id,
        type: "toggle",
        title: setting.title,
        description,
        defaultValue: setting.defaultValue === true,
      };
    }
    if (setting.type === "text" || setting.type === "secret") {
      const maxLength = setting.maxLength ?? 4_096;
      if (!Number.isInteger(maxLength) || maxLength < 1 || maxLength > 4_096) {
        throw new Error(`Setting ${id} maxLength must be between 1 and 4096`);
      }
      const placeholder = optionalText(
        setting.placeholder,
        `Setting placeholder for ${id}`,
        300,
      );
      if (setting.type === "secret") {
        if (setting.defaultValue !== undefined) {
          throw new Error(`Secret setting ${id} cannot define defaultValue`);
        }
        return {
          id,
          type: "secret",
          title: setting.title,
          description,
          placeholder,
          maxLength,
        };
      }
      if (setting.defaultValue !== undefined && typeof setting.defaultValue !== "string") {
        throw new Error(`Setting ${id} defaultValue must be text`);
      }
      const defaultValue = String(setting.defaultValue ?? "");
      if ([...defaultValue].length > maxLength) {
        throw new Error(`Setting ${id} defaultValue exceeds maxLength`);
      }
      return {
        id,
        type: "text",
        title: setting.title,
        description,
        placeholder,
        defaultValue,
        maxLength,
      };
    }
    if (setting.type === "select") {
      if (!Array.isArray(setting.options) || setting.options.length < 1 || setting.options.length > 128) {
        throw new Error(`Setting ${id} must have between 1 and 128 options`);
      }
      const seen = new Set<string>();
      const options = setting.options.map((option) => {
        if (!option || typeof option !== "object") {
          throw new Error(`Setting ${id} has an invalid option`);
        }
        assertText(option.value, `Setting option value for ${id}`, 1_024);
        assertText(option.label, `Setting option label for ${id}`, 200);
        if (seen.has(option.value)) {
          throw new Error(`Setting ${id} has duplicate option ${option.value}`);
        }
        seen.add(option.value);
        return { value: option.value, label: option.label };
      });
      if (setting.defaultValue !== undefined && typeof setting.defaultValue !== "string") {
        throw new Error(`Setting ${id} defaultValue must be text`);
      }
      const defaultValue = String(setting.defaultValue ?? options[0].value);
      if (!seen.has(defaultValue)) {
        throw new Error(`Setting ${id} defaultValue must match an option`);
      }
      return {
        id,
        type: "select",
        title: setting.title,
        description,
        defaultValue,
        options,
      };
    }
    throw new Error(`Setting ${id} has unsupported type ${String(setting.type)}`);
  });
}

function normalizeActions(value: unknown): unknown[] {
  if (value === undefined || value === null) return [];
  if (Array.isArray(value)) return value;
  if (
    typeof value === "object" &&
    value !== null &&
    Array.isArray((value as { actions?: unknown }).actions)
  ) {
    return (value as { actions: unknown[] }).actions;
  }
  return [value];
}

function assertStorageKey(value: unknown): asserts value is string {
  if (
    typeof value !== "string" ||
    value.trim() === "" ||
    value.length > 200 ||
    value.includes("\0")
  ) {
    throw new Error("Plugin storage key must be a non-empty string up to 200 characters");
  }
}

function emptyStorage(): Record<string, PluginJsonValue> {
  return Object.create(null) as Record<string, PluginJsonValue>;
}

function createPluginStorage(storageDirectory: string): PluginStorage {
  const storagePath = join(storageDirectory, "storage.json");
  let values: Record<string, PluginJsonValue> | undefined;
  let operations = Promise.resolve();

  const enqueue = <T>(operation: () => Promise<T>): Promise<T> => {
    const result = operations.then(operation, operation);
    operations = result.then(
      () => undefined,
      () => undefined,
    );
    return result;
  };
  const load = async (): Promise<Record<string, PluginJsonValue>> => {
    if (values) return values;
    let contents: Uint8Array;
    try {
      contents = new Uint8Array(await readFile(storagePath));
    } catch (error) {
      if ((error as { code?: string }).code === "ENOENT") {
        values = emptyStorage();
        return values;
      }
      throw error;
    }
    if (contents.byteLength > MAX_STORAGE_BYTES) {
      throw new Error("Plugin storage exceeds the 1 MiB limit");
    }
    let parsed: unknown;
    try {
      parsed = JSON.parse(Buffer.from(contents).toString("utf8"));
    } catch {
      throw new Error("Plugin storage contains invalid JSON");
    }
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
      throw new Error("Plugin storage must contain a JSON object");
    }
    const entries = Object.entries(parsed as Record<string, PluginJsonValue>);
    if (entries.length > MAX_STORAGE_ENTRIES) {
      throw new Error(`Plugin storage exceeds ${MAX_STORAGE_ENTRIES} entries`);
    }
    values = emptyStorage();
    for (const [key, value] of entries) {
      assertStorageKey(key);
      values[key] = value;
    }
    return values;
  };
  const persist = async (next: Record<string, PluginJsonValue>): Promise<void> => {
    const contents = JSON.stringify(next);
    if (Buffer.byteLength(contents) > MAX_STORAGE_BYTES) {
      throw new Error("Plugin storage exceeds the 1 MiB limit");
    }
    await mkdir(storageDirectory, { recursive: true });
    const temporaryPath = join(
      storageDirectory,
      `.storage-${process.pid}-${Date.now()}-${Math.random().toString(16).slice(2)}.tmp`,
    );
    try {
      await writeFile(temporaryPath, contents, { flag: "wx" });
      await rename(temporaryPath, storagePath);
    } finally {
      await rm(temporaryPath, { force: true });
    }
  };
  const cloneValue = (value: PluginJsonValue): PluginJsonValue =>
    structuredClone(value);

  return Object.freeze({
    get: <T = PluginJsonValue>(key: string): Promise<T | undefined> =>
      enqueue(async () => {
        assertStorageKey(key);
        const current = await load();
        if (!Object.hasOwn(current, key)) return undefined;
        return cloneValue(current[key]) as unknown as T;
      }),
    set: (key: string, value: PluginJsonValue): Promise<void> =>
      enqueue(async () => {
        assertStorageKey(key);
        let normalized: PluginJsonValue;
        try {
          const serialized = JSON.stringify(value);
          if (serialized === undefined) throw new Error();
          normalized = JSON.parse(serialized) as PluginJsonValue;
        } catch {
          throw new Error("Plugin storage values must be JSON-serializable");
        }
        const current = await load();
        const next = Object.assign(emptyStorage(), current);
        next[key] = normalized;
        if (Object.keys(next).length > MAX_STORAGE_ENTRIES) {
          throw new Error(`Plugin storage exceeds ${MAX_STORAGE_ENTRIES} entries`);
        }
        await persist(next);
        values = next;
      }),
    delete: (key: string): Promise<boolean> =>
      enqueue(async () => {
        assertStorageKey(key);
        const current = await load();
        if (!Object.hasOwn(current, key)) return false;
        const next = Object.assign(emptyStorage(), current);
        delete next[key];
        await persist(next);
        values = next;
        return true;
      }),
    clear: (): Promise<void> =>
      enqueue(async () => {
        await rm(storagePath, { force: true });
        values = emptyStorage();
      }),
  });
}

async function createPluginServices(
  source: PluginSource,
  dataRoot: string,
  cacheRoot: string,
): Promise<PluginServices> {
  if (!isAbsolute(dataRoot) || !isAbsolute(cacheRoot)) {
    throw new Error("Plugin storage paths must be absolute");
  }
  const storageDirectory = join(dataRoot, source.id);
  const dataDirectory = join(storageDirectory, "files");
  const cacheDirectory = join(cacheRoot, source.id);
  await Promise.all([
    mkdir(dataDirectory, { recursive: true }),
    mkdir(cacheDirectory, { recursive: true }),
  ]);
  return Object.freeze({
    storage: createPluginStorage(storageDirectory),
    paths: Object.freeze({ dataDirectory, cacheDirectory }),
  });
}

function createPluginContext(
  value: unknown,
  emittedActions: unknown[],
  services: PluginServices,
): PluginContext {
  const context =
    typeof value === "object" && value !== null && !Array.isArray(value)
      ? (value as Record<string, unknown>)
      : {};
  const settingValues =
    context.settings &&
    typeof context.settings === "object" &&
    !Array.isArray(context.settings)
      ? (context.settings as Record<string, string | boolean>)
      : {};
  const toast = (level: PluginToastLevel, message: unknown) => {
    assertText(message, "Toast message", 4_096);
    emittedActions.push({ type: "toast", level, message });
  };

  return {
    ...context,
    ...services,
    settings: Object.freeze({
      get: <T = string | boolean>(key: string): T | undefined => {
        assertId(key, "Setting ID");
        if (!Object.hasOwn(settingValues, key)) return undefined;
        return structuredClone(settingValues[key]) as T;
      },
    }),
    toasts: Object.freeze({
      info: (message: string) => toast("info", message),
      success: (message: string) => toast("success", message),
      warning: (message: string) => toast("warning", message),
      error: (message: string) => toast("error", message),
    }),
  };
}

async function handle(message: Record<string, unknown>): Promise<unknown> {
  if (message.type === "load") {
    const pluginSource = message.source as PluginSource;
    const source = await preparePlugin(
      pluginSource,
      String(message.bundleCacheRoot || ""),
    );
    pluginServices = await createPluginServices(
      pluginSource,
      String(message.pluginDataRoot || ""),
      String(message.pluginCacheRoot || ""),
    );
    const moduleUrl = `${pathToFileURL(source.path).href}?termy=${source.cacheKey}`;
    let loaded: Record<string, unknown>;
    try {
      loaded = await import(moduleUrl);
    } catch (error) {
      await rm(source.path, { force: true });
      throw error;
    }
    return normalizePlugin(loaded.default, source);
  }
  if (message.type === "invoke") {
    if (!plugin) throw new Error("Plugin is not loaded");
    if (!pluginServices) throw new Error("Plugin services are unavailable");
    const commandId = String(message.commandId || "");
    const run = commandHandlers.get(commandId);
    if (!run) throw new Error(`Command ${commandId} is not registered`);
    const emittedActions: unknown[] = [];
    const value = await run({
      inputs: (message.inputs || {}) as Record<string, unknown>,
      context: createPluginContext(message.context, emittedActions, pluginServices),
    });
    return { actions: [...emittedActions, ...normalizeActions(value)] };
  }
  throw new Error(`Unknown Worker request ${String(message.type)}`);
}

self.onmessage = (event: MessageEvent<Record<string, unknown>>) => {
  const message = event.data;
  queue = queue.then(async () => {
    try {
      const result = await handle(message);
      postMessage({ id: message.id, ok: true, result });
    } catch (error) {
      process.stderr.write(
        `[termy plugin ${process.env.TERMY_PLUGIN_ID || "unknown"}] ${errorMessage(error)}\n`,
      );
      postMessage({ id: message.id, ok: false, error: errorMessage(error) });
    }
  });
};

export {};
