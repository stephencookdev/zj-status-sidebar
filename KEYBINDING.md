# Keybinding Configuration

## Tab Mode Toggle (Ctrl+t, t)

The toggle keybinding is configured in your Zellij config to send a message to the plugin:

```kdl
keybinds {
    tab {
        bind "t" {
            MessagePlugin "file:~/.config/zellij/plugins/zj-status-sidebar.wasm" {
                name "toggle_collapse"
            }
            SwitchToMode "Normal"
        }
    }
}
```

## Visual Toggle

You can also click on the title bar to toggle between expanded and collapsed views.

## Manual Layout Control

For proper pane resizing, you can switch between layouts:

### Expanded view (20% width):
```bash
zellij action new-tab --layout left-sidebar
```

### Collapsed view (3 characters wide):
```bash
zellij action new-tab --layout left-sidebar-collapsed
```

### Create aliases for convenience:
```bash
alias zjse='zellij action new-tab --layout left-sidebar'        # sidebar expanded
alias zjsc='zellij action new-tab --layout left-sidebar-collapsed'  # sidebar collapsed
```

Note: Zellij plugins cannot directly resize their panes, so manual layout switching or using different layouts is the best approach for changing the sidebar width.