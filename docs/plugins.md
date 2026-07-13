# TypeScript plugins

Termy plugins add commands to the command palette with a small `plugin.json` manifest and a plain TypeScript entrypoint. A plugin does not need a package or handwritten build step: put both files in its plugin directory and export one `definePlugin(...)` value from the TypeScript source.

## Requirements

Plugins run in a persistent external [Bun](https://bun.sh/) host. Install Bun before opening the command palette with plugins present, or set `TERMY_BUN_PATH` to the absolute path of a Bun executable.

A command handler is normal Bun code, not a restricted expression language. It can use async functions, `fetch`, `Bun.*`, Node-compatible standard-library APIs, files, subprocesses, network requests, and local relative TypeScript imports. Returned actions are the typed bridge back into Termy's native command palette, terminal, clipboard, browser, and toast UI.

Termy resolves Bun in this order:

1. `TERMY_BUN_PATH`
2. `bun` or `bun.exe` beside the Termy executable
3. `bun` on `PATH`
4. `$HOME/.bun/bin/bun` on macOS and Linux, or `%USERPROFILE%\.bun\bin\bun.exe` on Windows
5. `/opt/homebrew/bin/bun`
6. `/usr/local/bin/bun`

Termy starts one long-lived Bun host and gives each plugin its own Worker. This keeps command invocation warm while allowing a timed-out or failed plugin Worker to be stopped without taking down the terminal UI.

## Plugin directory

The easiest install path is **Settings → Plugins → Install from folder**. Choose a plugin folder containing `plugin.json` and its TypeScript entrypoint; Termy validates the source, copies it into the managed plugin directory, and shows its name, version, and enabled state. The same screen can refresh the inventory, open the plugin directory, enable or disable a plugin, and uninstall it with confirmation.

The CLI can scaffold a plugin and install trusted source directly from GitHub:

```sh
termy plugin init my-plugin
termy plugin add https://github.com/example/termy-plugins --path my-plugin
termy plugin status my-plugin
termy plugin disable my-plugin
termy plugin enable my-plugin
termy plugin update my-plugin
termy plugin uninstall my-plugin
```

`add` also has the `install` alias, while `remove` has the `uninstall` alias. Repository URLs, `/tree/<ref>/<path>` URLs, `--ref`, and `--path` are supported. Termy resolves the selected ref to a full commit, downloads only regular files, validates the manifest and source limits, and saves the repository, requested ref, plugin subdirectory, and pinned revision for later status and updates. A repository containing multiple valid plugins requires `--path` so Termy never guesses which one to install.

GitHub installation never clones the repository, runs package scripts, installs dependencies, or evaluates source during installation. Plugins are still trusted code when the command palette loads them through Bun, so the CLI shows a warning and requires confirmation; automation must pass `--yes` explicitly.

You can also manage the directory manually:

Create one directory per plugin under Termy's config directory:

```text
termy/
└── plugins/
    ├── termy.d.ts
    ├── .termy-cache/
    │   └── bundles/
    │       └── git-tools/
    │           └── <content-hash>.mjs
    └── git-tools/
        ├── plugin.json
        └── plugin.ts
```

The config directory is `$XDG_CONFIG_HOME/termy` when `XDG_CONFIG_HOME` is set, `~/.config/termy` otherwise on macOS and Linux, and `%APPDATA%\termy` on Windows.

Termy manages `plugins/termy.d.ts` and everything under `plugins/.termy-cache`; do not edit either. The declarations provide the global `definePlugin` function and ambient `TermyPlugin` types for editor completion, so plugin source does not import an SDK package.

## Manifest

Every plugin directory contains a `plugin.json` manifest:

```json
{
  "apiVersion": 1,
  "id": "hello",
  "name": "Hello",
  "version": "1.0.0"
}
```

| Field | Required | Description |
| --- | --- | --- |
| `apiVersion` | yes | Plugin API version; v1 requires `1`. |
| `id` | yes | Stable plugin ID. It should match the directory name. |
| `name` | yes | Human-readable plugin name shown by Termy. |
| `version` | no | Version shown in Settings when the plugin is managed there. |
| `main` | no | TypeScript entrypoint relative to the plugin directory. Defaults to `plugin.ts`. |

Use lowercase letters, numbers, and hyphens for IDs so saved references remain stable when a display name changes. `main` must resolve to a regular file inside the plugin directory; out-of-root paths and symlinks are rejected.

## Minimal plugin

`plugin.json`:

```json
{
  "apiVersion": 1,
  "id": "hello",
  "name": "Hello"
}
```

`plugin.ts`:

```ts
export default definePlugin({
  commands: [
    {
      id: "say-hello",
      title: "Hello: Greet me",
      keywords: ["hello", "example"],
      icon: "info",
      run() {
        return {
          type: "toast",
          level: "success",
          message: "Hello from Termy",
        };
      },
    },
  ],
} satisfies TermyPlugin);
```

The manifest owns API and identity metadata. `definePlugin` owns runtime behavior and therefore contains only `commands`; do not duplicate `apiVersion`, `id`, `name`, `version`, or `main` in `plugin.ts`.

## Command fields

Each command accepts these fields:

| Field | Required | Description |
| --- | --- | --- |
| `id` | yes | Stable command ID, unique within the plugin. |
| `title` | yes | Label shown in the command palette. |
| `keywords` | no | Extra strings matched by palette search. |
| `status` | no | Compact status text shown on the palette row. |
| `enabled` | no | Set to `false` to keep the command visible but unavailable. |
| `disabledReason` | no | Explanation shown for a disabled command. |
| `icon` | no | One of `command`, `play`, `terminal`, `folder`, `link`, `clipboard`, `settings`, or `info`. |
| `inputs` | no | Text, select, and confirm prompts collected before `run`. |
| `run` | yes | Async or synchronous command handler. |

Termy namespaces runtime commands as `<plugin-id>.<command-id>`, so command IDs only need to be unique inside their plugin.

## Imports and build cache

Plugin source can use local relative imports inside its plugin directory:

```ts
import { formatMessage } from "./messages.ts";
```

V1 accepts local relative imports plus Bun and Node built-ins such as `bun` and `node:fs`. Every local import must resolve to a regular file inside the plugin directory. Package imports, absolute or out-of-root source paths, and symlinks are rejected.

Termy does not run `bun install` or automatically install missing packages. Keep the complete local source tree inside the plugin directory.

When the command palette opens, Termy fingerprints each manifest and plugin source tree. A new content hash is bundled once with `Bun.build({ target: "bun" })` and written to:

```text
plugins/.termy-cache/bundles/<plugin-id>/<content-hash>.mjs
```

The bundle includes local relative TypeScript imports. An unchanged plugin reuses its cached bundle; a changed manifest or source tree produces a new hash and bundle, then Termy imports it in that plugin's warm Worker.

Termy deliberately does not use `bun build --compile`. A compiled plugin executable embeds the Bun runtime in every plugin artifact, multiplying disk and startup overhead. Cached `.mjs` bundles share the one persistent Bun host while still avoiding repeated transpilation.

## Inputs

Inputs appear sequentially in their array order. Once Termy has collected and validated every value, it calls `run({ inputs, context })`. `inputs` is keyed by input ID and contains strings or booleans.

```ts
inputs: [
  {
    id: "label",
    type: "text",
    label: "Label",
    placeholder: "Release name",
    defaultValue: "next",
    required: true,
    maxLength: 80,
  },
  {
    id: "target",
    type: "select",
    label: "Target",
    required: true,
    options: [
      { value: "debug", label: "Debug", keywords: ["dev"] },
      { value: "release", label: "Release", keywords: ["production"] },
    ],
  },
  {
    id: "confirmed",
    type: "confirm",
    label: "Run now?",
    defaultValue: true,
  },
]
```

Text inputs support `placeholder`, `defaultValue`, `required`, and `maxLength`. Select inputs support `placeholder`, `defaultValue`, `required`, and a fixed `options` array. Confirm inputs return a boolean and support `defaultValue`.

Treat text input as untrusted. Do not interpolate it directly into a shell command; prefer a select input mapped to fixed commands, or apply quoting appropriate for the target shell.

## Context

Every handler receives the current Termy context:

```ts
type PluginContext = {
  workingDirectory?: string;
  activeCommand?: string;
  platform: "macos" | "linux" | "windows";
  appVersion: string;
};
```

`workingDirectory` and `activeCommand` are absent when the active terminal cannot provide them. Plugins should handle that case instead of assuming both values exist.

## Actions

A handler can return nothing, one action, an action array, or `{ actions: [...] }`. Async handlers may return the same values through a Promise.

| Action | Shape | Effect |
| --- | --- | --- |
| Run in the terminal | `{ type: "terminal.run", command, workingDirectory? }` | Run a shell command, optionally in a specific directory. |
| Run a Termy command | `{ type: "termy.command", command }` | Invoke a built-in Termy command by its stable command name. |
| Copy text | `{ type: "clipboard.write", text }` | Write text to the system clipboard. |
| Open a URL | `{ type: "url.open", url }` | Open an `http` or `https` URL with the system browser. |
| Show a toast | `{ type: "toast", level, message }` | Show an `info`, `success`, `warning`, or `error` notification. |

Actions run in returned order. Keep handlers focused and return only the effects the command needs.

## Reloading and failures

Termy checks plugin content when the command palette opens. When `plugin.json`, `plugin.ts`, or another local source file changes, Termy creates or reuses the matching bundle, replaces that plugin's Worker, and refreshes the command list; restarting Termy is unnecessary.

Disabling a plugin in Settings keeps its files installed but removes its commands the next time the command palette refreshes. Enabling it makes the commands available again. Uninstalling removes Termy's managed copy, not the source folder you originally selected.

Plugin loading and command execution have timeouts. A thrown error or timeout is contained to that plugin Worker and reported in Termy, while other plugins and the terminal keep running. A subprocess started by plugin code can outlive its Worker, so plugins that spawn processes must stop them themselves when cancellation matters. Plugins share the persistent host transport; if that transport exits, Termy discards it and rebuilds the host and Workers on the next plugin refresh instead of taking down the app. Fix a failed plugin and reopen the palette to load it again.

## Security and v1 limits

Plugins are trusted local code. Worker isolation, import validation, and timeouts improve reliability, but they are not a security sandbox: after loading, a plugin runs through Bun with your user account's access to files, network, and processes. Only install plugins whose source you trust.

The v1 runtime deliberately supports command-palette commands only. It does not provide plugin keybindings, custom UI, build hooks, package imports, or automatic package installation. Bun is launched with dependency installation and environment-file loading disabled; local relative TypeScript imports are bundled from the plugin directory, while Bun and Node built-ins remain available at runtime.

See [`examples/plugins/git-tools/plugin.json`](../examples/plugins/git-tools/plugin.json) and [`plugin.ts`](../examples/plugins/git-tools/plugin.ts) for a safe command example that maps a select value to a fixed shell command.
