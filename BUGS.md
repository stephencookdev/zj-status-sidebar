# Known Bugs and Limitations

## Plugin Crash in Narrow Mode
**Issue**: The plugin crashes when the pane width is very narrow (3-6 characters).
**Status**: Attempted fix for string slicing bounds, but crash still occurs.
**Next Steps**: Need to add more defensive checks for all string operations and ANSI escape sequences at narrow widths.

## Swap Layout Not Global
**Issue**: Zellij's swap layouts (NextSwapLayout) only apply to the current tab, not all tabs.
**Status**: This is a fundamental limitation of Zellij - swap layouts are per-tab by design.
**Attempted Solutions**:
- Added plugin-to-plugin messaging to sync collapse state
- Added filesystem state persistence
**Next Steps**: 
- Consider filing a feature request with Zellij for global swap layouts
- Alternative: Create a Zellij CLI script that switches to each tab and applies the swap layout

## Visual State Sync
**Issue**: The visual collapse state should sync across all plugin instances in different tabs.
**Status**: Implemented message broadcasting, but needs testing to verify it works correctly.
**Next Steps**: Debug the plugin message passing system to ensure all instances receive and process the sync messages.

## Workarounds
For now, users need to:
1. Manually press Ctrl+t,t in each tab to resize panes
2. The visual state (expanded/collapsed text) should sync, but pane sizes won't