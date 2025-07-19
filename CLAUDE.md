# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

zj-status-sidebar is a Zellij plugin written in Rust that provides an enhanced status bar for the Zellij terminal multiplexer. It's compiled to WebAssembly (wasm32-wasi) and focuses on providing a clean, opinionated status bar experience.

## Common Development Commands

```bash
# Build the plugin (development)
cargo build

# Build the plugin (release)
cargo build --release

# Check code compilation
cargo check

# Run pre-commit checks
node scripts/check.js
```

The compiled WASM files are located at:
- Development: `target/wasm32-wasi/debug/zj-status-bar.wasm`
- Release: `target/wasm32-wasi/release/zj-status-bar.wasm`

## Architecture

### Core Structure
The plugin follows Zellij's event-driven plugin architecture:

1. **State Management** (`src/main.rs`): Central `State` struct implements `ZellijPlugin` trait and manages:
   - Pane information
   - Tab alerts (HashMap<usize, Alert>)
   - Tab list and active tab index
   - Mode information
   - Rendered tab line parts

2. **Tab Rendering** (`src/tab.rs`): Handles individual tab styling with:
   - Active/inactive states (italics for inactive)
   - Alert indicators (success/failure colors)
   - Mouse interaction regions

3. **Line Composition** (`src/line.rs`): Assembles the complete status bar from parts

### Event System
The plugin subscribes to these Zellij events:
- TabUpdate, PaneUpdate, ModeUpdate
- Mouse events for tab interaction
- Timer events for alert animations
- PermissionRequestResult for initial setup

### Tab Alerts Feature
Requires shell integration via the `zw` function. The alert system:
- Tracks command exit codes in background tabs
- Shows green (success) or red (failure) indicators
- Uses 1-second timers for animations
- Syncs state across plugin instances via pipe messages

### Plugin Communication
- Uses Zellij's pipe messaging for state synchronization
- Handles "zj-status-bar-sync-tab-alerts" messages
- Serializes/deserializes alert state with serde_json

## Development Notes

- Target platform is `wasm32-wasi` - ensure Rust toolchain includes this target
- No external test framework - rely on Rust's built-in testing
- Pre-commit hook runs `cargo check` automatically
- Mouse support includes click-to-switch and scroll navigation
- Plugin requests permissions on first load and becomes unselectable after