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

type TermyPluginContext = {
  workingDirectory?: string;
  activeCommand?: string;
  platform: "macos" | "linux" | "windows";
  appVersion: string;
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

type TermyPluginCommand = {
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
    context: TermyPluginContext;
  }): TermyPluginResult | Promise<TermyPluginResult>;
};

type TermyPlugin = {
  commands: TermyPluginCommand[];
};

declare function definePlugin<T extends TermyPlugin>(plugin: T): T;
