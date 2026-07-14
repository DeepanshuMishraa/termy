// Managed ambient declarations for plain TypeScript plugins.
type TermyPluginIcon =
  | "command"
  | "play"
  | "terminal"
  | "folder"
  | "link"
  | "clipboard"
  | "settings"
  | "info";

type TermyPluginInputValue = string | boolean;

type TermyPluginTextInput = {
  id: string;
  type: "text";
  label: string;
  placeholder?: string;
  defaultValue?: string;
  required?: boolean;
  maxLength?: number;
};

type TermyPluginSelectInput = {
  id: string;
  type: "select";
  label: string;
  placeholder?: string;
  defaultValue?: string;
  required?: boolean;
  options: Array<{
    value: string;
    label: string;
    keywords?: string[];
    status?: string;
  }>;
};

type TermyPluginConfirmInput = {
  id: string;
  type: "confirm";
  label: string;
  defaultValue?: boolean;
};

type TermyPluginInput =
  | TermyPluginTextInput
  | TermyPluginSelectInput
  | TermyPluginConfirmInput;

type TermyPluginToasts = {
  info(message: string): void;
  success(message: string): void;
  warning(message: string): void;
  error(message: string): void;
};

type TermyPluginJsonValue =
  | null
  | boolean
  | number
  | string
  | TermyPluginJsonValue[]
  | { [key: string]: TermyPluginJsonValue };

type TermyPluginStorage = {
  get<T = TermyPluginJsonValue>(key: string): Promise<T | undefined>;
  set(key: string, value: TermyPluginJsonValue): Promise<void>;
  delete(key: string): Promise<boolean>;
  clear(): Promise<void>;
};

type TermyPluginToggleSetting = {
  type: "toggle";
  title: string;
  description?: string;
  defaultValue?: boolean;
};

type TermyPluginTextSetting = {
  type: "text";
  title: string;
  description?: string;
  placeholder?: string;
  defaultValue?: string;
  maxLength?: number;
};

type TermyPluginSelectSetting = {
  type: "select";
  title: string;
  description?: string;
  defaultValue?: string;
  options: Array<{ value: string; label: string }>;
};

type TermyPluginSecretSetting = {
  type: "secret";
  title: string;
  description?: string;
  placeholder?: string;
  maxLength?: number;
};

type TermyPluginSetting =
  | TermyPluginToggleSetting
  | TermyPluginTextSetting
  | TermyPluginSelectSetting
  | TermyPluginSecretSetting;

type TermyPluginSettingValue<T extends TermyPluginSetting> =
  T extends TermyPluginToggleSetting ? boolean : string;

type TermyPluginSettings<
  T extends Record<string, TermyPluginSetting> = Record<string, TermyPluginSetting>,
> = {
  get<K extends keyof T & string>(key: K): TermyPluginSettingValue<T[K]> | undefined;
};

type TermyPluginContext<
  T extends Record<string, TermyPluginSetting> = Record<string, TermyPluginSetting>,
> = {
  readonly workingDirectory?: string;
  readonly activeCommand?: string;
  readonly selectedText?: string;
  readonly selectedTextTruncated: boolean;
  readonly shell: string;
  readonly runtime: "native" | "tmux";
  readonly activeTab?: {
    readonly index: number;
    readonly title: string;
    readonly paneCount: number;
  };
  readonly activePane?: {
    readonly index: number;
    readonly kind: "terminal" | "browser";
  };
  readonly platform: "macos" | "linux" | "windows";
  readonly appVersion: string;
  readonly settings: TermyPluginSettings<T>;
  readonly toasts: TermyPluginToasts;
  readonly storage: TermyPluginStorage;
  readonly paths: {
    readonly dataDirectory: string;
    readonly cacheDirectory: string;
  };
};

type TermyPluginAction =
  | { type: "terminal.run"; command: string; workingDirectory?: string }
  | { type: "termy.command"; command: string }
  | { type: "clipboard.write"; text: string }
  | { type: "url.open"; url: string }
  | {
      type: "toast";
      level: "info" | "success" | "warning" | "error";
      message: string;
    };

type TermyPluginResult =
  | void
  | TermyPluginAction
  | TermyPluginAction[]
  | { actions: TermyPluginAction[] };

type TermyPluginEvent =
  | { readonly type: "terminal.ready" }
  | { readonly type: "tab.activated"; readonly previousTabIndex?: number }
  | {
      readonly type: "workingDirectory.changed";
      readonly previousWorkingDirectory?: string;
      readonly workingDirectory?: string;
    }
  | {
      readonly type: "command.finished";
      readonly command?: string;
      readonly exitCode?: number;
      readonly durationMs?: number;
    };

type TermyPluginEvents<
  T extends Record<string, TermyPluginSetting> = Record<string, TermyPluginSetting>,
> = {
  readonly [K in TermyPluginEvent["type"]]?: (request: {
    readonly event: Extract<TermyPluginEvent, { type: K }>;
    readonly context: TermyPluginContext<T>;
  }) => TermyPluginResult | Promise<TermyPluginResult>;
};

type TermyPluginCommand<
  T extends Record<string, TermyPluginSetting> = Record<string, TermyPluginSetting>,
> = {
  id: string;
  title: string;
  keywords?: string[];
  status?: string;
  enabled?: boolean;
  disabledReason?: string;
  icon?: TermyPluginIcon;
  inputs?: TermyPluginInput[];
  timeoutMs?: number;
  run(request: {
    inputs: Record<string, TermyPluginInputValue>;
    context: TermyPluginContext<T>;
  }): TermyPluginResult | Promise<TermyPluginResult>;
};

type TermyPlugin<
  T extends Record<string, TermyPluginSetting> = Record<string, TermyPluginSetting>,
> = {
  settings?: T;
  commands: TermyPluginCommand<T>[];
  events?: TermyPluginEvents<T>;
};

declare function definePlugin<const T extends Record<string, TermyPluginSetting>>(
  plugin: TermyPlugin<T>,
): TermyPlugin<T>;
