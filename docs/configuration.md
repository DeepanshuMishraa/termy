# Configuration

Termy uses a text config file inspired by Ghostty:

- `~/.config/termy/config.txt`

## Recommended Starter Config

Most users only need this:

```txt
theme = termy
use_tabs = true
tab_title_mode = smart
tab_title_shell_integration = true
```

## Tab Titles

### Basic (recommended)

`tab_title_mode`
- Default: `smart`
- Values: `smart`, `shell`, `explicit`, `static`
- What it does: chooses a sensible title strategy.

Mode presets:
- `smart`: `manual, explicit, shell, fallback`
- `shell`: `manual, shell, fallback`
- `explicit`: `manual, explicit, fallback`
- `static`: `manual, fallback`

`tab_title_shell_integration`
- Default: `true`
- Values: `true`/`false`
- What it does: exports `TERMY_*` environment variables for shell hooks.

`tab_title_fallback`
- Default: `Terminal`
- Values: non-empty string
- What it does: fallback title if higher-priority sources are empty.

Note:
- Termy applies a built-in short delay before showing `command:...` titles to reduce flash for fast commands.

### Advanced (optional)

`tab_title_priority`
- Default: unset (derived from `tab_title_mode`)
- Values: comma-separated list using `manual`, `explicit`, `shell`, `fallback`
- What it does: exact source order override. If set, it wins over `tab_title_mode`.

`tab_title_explicit_prefix`
- Default: `termy:tab:`
- Values: string prefix
- What it does: marks explicit payloads in OSC title updates.

`tab_title_prompt_format`
- Default: `{cwd}`
- Values: template string with optional `{cwd}` and `{command}` placeholders
- What it does: formats explicit `prompt:...` payloads.

`tab_title_command_format`
- Default: `{command}`
- Values: template string with optional `{cwd}` and `{command}` placeholders
- What it does: formats explicit `command:...` payloads.

Explicit payload examples:
- `termy:tab:prompt:~/projects/termy`
- `termy:tab:command:cargo test`
- `termy:tab:title:Deploy`

## All Config Options

`theme`
- Default: `termy`
- Values: `termy`, `tokyonight`, `catppuccin`, `dracula`, `gruvbox`, `nord`, `solarized`, `onedark`, `monokai`, `material`, `palenight`, `tomorrow`, `oceanic`

`working_dir`
- Default: unset
- Values: path string (`~` supported)

`use_tabs`
- Default: `false`
- Values: `true`/`false`

`tab_title_mode`
- Default: `smart`
- Values: `smart`, `shell`, `explicit`, `static`

`tab_title_shell_integration`
- Default: `true`
- Values: `true`/`false`

`tab_title_fallback`
- Default: `Terminal`
- Values: non-empty string

`tab_title_priority`
- Default: unset (derived from `tab_title_mode`)
- Values: `manual`, `explicit`, `shell`, `fallback` (comma-separated)

`tab_title_explicit_prefix`
- Default: `termy:tab:`
- Values: string

`tab_title_prompt_format`
- Default: `{cwd}`
- Values: template string

`tab_title_command_format`
- Default: `{command}`
- Values: template string

`window_width`
- Default: `1100`
- Values: positive number

`window_height`
- Default: `720`
- Values: positive number

`font_family`
- Default: `JetBrains Mono`
- Values: font family name

`font_size`
- Default: `14`
- Values: positive number

`padding_x`
- Default: `12`
- Values: non-negative number

`padding_y`
- Default: `8`
- Values: non-negative number

## Shell Integration Snippets

If `tab_title_shell_integration = true`, Termy exports:

- `TERMY_SHELL_INTEGRATION=1`
- `TERMY_TAB_TITLE_PREFIX=<tab_title_explicit_prefix>`

### zsh (`~/.zshrc`)

```sh
if [[ "${TERMY_SHELL_INTEGRATION:-0}" == "1" ]]; then
  _termy_emit_tab_title() {
    local kind="$1"
    shift
    local payload="$*"
    local prefix="${TERMY_TAB_TITLE_PREFIX:-termy:tab:}"
    payload=${payload//$'\n'/ }
    payload=${payload//$'\r'/ }
    printf '\033]2;%s%s:%s\007' "$prefix" "$kind" "$payload"
  }

  _termy_prompt_title() {
    _termy_emit_tab_title "prompt" "${PWD/#$HOME/~}"
  }

  _termy_command_title() {
    _termy_emit_tab_title "command" "$1"
  }

  autoload -Uz add-zsh-hook
  add-zsh-hook precmd _termy_prompt_title
  add-zsh-hook preexec _termy_command_title
fi
```

### bash (`~/.bashrc`)

```sh
if [[ "${TERMY_SHELL_INTEGRATION:-0}" == "1" ]]; then
  _termy_emit_tab_title() {
    local kind="$1"
    shift
    local payload="$*"
    local prefix="${TERMY_TAB_TITLE_PREFIX:-termy:tab:}"
    payload=${payload//$'\n'/ }
    payload=${payload//$'\r'/ }
    printf '\033]2;%s%s:%s\007' "$prefix" "$kind" "$payload"
  }

  _termy_preexec() {
    [[ "$BASH_COMMAND" == "$PROMPT_COMMAND" ]] && return
    _termy_emit_tab_title "command" "$BASH_COMMAND"
  }

  _termy_precmd() {
    _termy_emit_tab_title "prompt" "${PWD/#$HOME/~}"
  }

  trap '_termy_preexec' DEBUG
  PROMPT_COMMAND="_termy_precmd${PROMPT_COMMAND:+;$PROMPT_COMMAND}"
fi
```

### fish (`~/.config/fish/config.fish`)

```fish
if test "$TERMY_SHELL_INTEGRATION" = "1"
  function __termy_emit_tab_title
    set kind $argv[1]
    set payload (string join " " $argv[2..-1])
    set payload (string replace -a \n " " $payload)
    set payload (string replace -a \r " " $payload)
    set prefix (set -q TERMY_TAB_TITLE_PREFIX; and echo $TERMY_TAB_TITLE_PREFIX; or echo "termy:tab:")
    printf '\e]2;%s%s:%s\a' $prefix $kind $payload
  end

  function __termy_preexec --on-event fish_preexec
    __termy_emit_tab_title command $argv
  end

  function __termy_prompt --on-event fish_prompt
    set cwd (string replace -r "^$HOME" "~" $PWD)
    __termy_emit_tab_title prompt $cwd
  end
end
```
