# Keybindings

Termy keybindings use Ghostty-style trigger overrides via repeated `keybind` lines in `~/.config/termy/config.txt`.

## Default Keybinds

These are installed by default before any user `keybind` directives are applied:

### Global Actions

- `secondary-q` -> `quit`
- `secondary-,` -> `open_config`

### Terminal Actions

- `secondary-p` -> `toggle_command_palette`
- `secondary-t` -> `new_tab`
- `secondary-w` -> `close_tab`
- `secondary-=` -> `zoom_in`
- `secondary--` -> `zoom_out`
- `secondary-0` -> `zoom_reset`

### Copy/Paste Defaults

- macOS/Windows: `secondary-c` -> `copy`, `secondary-v` -> `paste`
- Linux/other: `ctrl-shift-c` -> `copy`, `ctrl-shift-v` -> `paste`

`secondary` maps to `cmd` on macOS and `ctrl` on non-macOS platforms.

## Config Syntax

Supported forms:

- `keybind = clear`
- `keybind = <trigger>=<action>`
- `keybind = <trigger>=unbind`

Behavior:

- Directives are applied in file order.
- Later lines win for the same trigger.
- `clear` removes all defaults before later lines are applied.
- `unbind` removes the current mapping for a trigger.
- Invalid lines are ignored (with warnings).

Configurable actions:

- `quit`
- `open_config`
- `toggle_command_palette`
- `new_tab`
- `close_tab`
- `copy`
- `paste`
- `zoom_in`
- `zoom_out`
- `zoom_reset`

## Customization Examples

### 1) Override one default

```txt
keybind = cmd-p=toggle_command_palette
```

### 2) Remove one default

```txt
keybind = cmd-w=unbind
```

### 3) Start from scratch

```txt
keybind = clear
keybind = cmd-p=toggle_command_palette
keybind = cmd-t=new_tab
keybind = cmd-w=close_tab
keybind = cmd-c=copy
keybind = cmd-v=paste
```

### 4) Linux-style copy/paste on any platform

```txt
keybind = clear
keybind = ctrl-shift-c=copy
keybind = ctrl-shift-v=paste
```

### 5) Rebind zoom controls

```txt
keybind = cmd-i=zoom_in
keybind = cmd-o=zoom_out
keybind = cmd-u=zoom_reset
```

### 6) Use `secondary` for cross-platform configs

```txt
keybind = secondary-p=toggle_command_palette
keybind = secondary-t=new_tab
```
