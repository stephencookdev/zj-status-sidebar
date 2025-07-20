mod line;
mod tab;
mod names;

use std::cmp::{max, min};
use std::collections::BTreeMap;
use std::collections::HashMap;

use zellij_tile::prelude::*;
use zellij_tile_utils::style;

use serde::{Deserialize, Serialize};
use crate::names::NameCache;

#[derive(Debug, Default)]
pub struct LinePart {
    part: String,
    len: usize,
    tab_index: Option<usize>,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
struct TabAlert {
    success: bool,
    alternate_color: bool,
}

struct State {
    pane_info: PaneManifest,
    tab_alerts: HashMap<usize, TabAlert>,
    tabs: Vec<TabInfo>,
    active_tab_idx: usize,
    mode_info: ModeInfo,
    tab_line: Vec<LinePart>,
    name_cache: NameCache,
    collapsed: bool,
    rows: usize,
}

impl Default for State {
    fn default() -> Self {
        Self {
            pane_info: PaneManifest::default(),
            tab_alerts: HashMap::new(),
            tabs: Vec::new(),
            active_tab_idx: 0,
            mode_info: ModeInfo::default(),
            tab_line: Vec::new(),
            name_cache: NameCache::new(),
            collapsed: false,
            rows: 0,
        }
    }
}

static ARROW_SEPARATOR: &str = "";

register_plugin!(State);

impl ZellijPlugin for State {
    fn load(&mut self, _configuration: BTreeMap<String, String>) {
        request_permission(&[
            PermissionType::ReadApplicationState,
            PermissionType::ChangeApplicationState,
            PermissionType::MessageAndLaunchOtherPlugins,
        ]);
        subscribe(&[
            EventType::TabUpdate,
            EventType::PaneUpdate,
            EventType::ModeUpdate,
            EventType::Mouse,
            EventType::Key,
            EventType::PermissionRequestResult,
            EventType::Timer,
        ]);
        
        // Check if sidebar was collapsed in previous session
        let state_file = "/tmp/.zj-sidebar-collapsed";
        self.collapsed = std::path::Path::new(state_file).exists();
        
        // Set as selectable on load so user can accept/deny perms.
        // After the first load, if the user allowed access, the perm event handler
        // in `update` will always set it as unselectable.
        set_selectable(true);
    }

    fn update(&mut self, event: Event) -> bool {
        let mut should_render = false;
        match event {
            Event::PaneUpdate(pane_info) => {
                self.pane_info = pane_info;
            }
            Event::ModeUpdate(mode_info) => {
                if self.mode_info != mode_info {
                    should_render = true;
                }
                self.mode_info = mode_info
            }
            Event::Timer(_) => {
                // Skip event if there's no alerts.
                // This ensures the last timer fired after visited the last tab with an alert don't
                // cause an infinite re-render loop.
                if !self.tab_alerts.is_empty() {
                    for tab_alert in self.tab_alerts.values_mut() {
                        *tab_alert = TabAlert {
                            success: tab_alert.success,
                            alternate_color: !tab_alert.alternate_color,
                        }
                    }

                    set_timeout(1.0);
                    should_render = true;

                    // Broadcast the state of tab alerts to all instances of `zj-status-bar` for new
                    // instances to "catch up" on previous alerts.
                    pipe_message_to_plugin(
                        MessageToPlugin::new("zj-status-sidebar:plugin:tab_alert:broadcast")
                            .with_plugin_url("zellij:OWN_URL")
                            .with_payload(serde_json::to_string(&self.tab_alerts).unwrap()),
                    )
                }
            }
            Event::TabUpdate(tabs) => {
                if let Some(active_tab_index) = tabs.iter().position(|t| t.active) {
                    // tabs are indexed starting from 1 so we need to add 1
                    let active_tab_idx = active_tab_index + 1;
                    if self.active_tab_idx != active_tab_idx || self.tabs != tabs {
                        self.tab_alerts.remove(&active_tab_index);
                        should_render = true;
                    }
                    self.active_tab_idx = active_tab_idx;
                    self.tabs = tabs;
                } else {
                    eprintln!("Could not find active tab.");
                }
            }
            Event::Key(key) => {
                // Check if we're in Tab mode (after pressing Ctrl+t)
                if self.mode_info.mode == InputMode::Tab {
                    // Handle 't' key in Tab mode to toggle collapsed state
                    if let Key::Char('t') = key {
                        self.collapsed = !self.collapsed;
                        should_render = true;
                        // Switch back to normal mode
                        switch_to_input_mode(&InputMode::Normal);
                    }
                }
            }
            Event::Mouse(me) => match me {
                Mouse::LeftClick(row, _col) => {
                    if row == 0 {
                        // Click on title row toggles collapse
                        self.collapsed = !self.collapsed;
                        should_render = true;
                        
                        // Write state to filesystem for resize script
                        let state_file = "/tmp/.zj-sidebar-collapsed";
                        if self.collapsed {
                            std::fs::write(state_file, "1").ok();
                        } else {
                            std::fs::remove_file(state_file).ok();
                        }
                        
                        // Broadcast collapse state to all plugin instances
                        pipe_message_to_plugin(
                            MessageToPlugin {
                                plugin_url: None,
                                plugin_config: BTreeMap::new(),
                                message_name: "sync_collapse_state".to_string(),
                                message_payload: Some(self.collapsed.to_string()),
                                message_args: BTreeMap::new(),
                                new_plugin_args: None,
                                destination_plugin_id: None,
                            }
                        );
                    } else if row >= 2 {
                        // All tabs are 3 lines tall
                        let tab_height = 3;
                        // First 2 rows are header (title, spacer), tabs start at row 2 (0-indexed)
                        let tab_idx = (row as usize - 2) / tab_height;
                        if tab_idx < self.tabs.len() {
                            let tab_number = tab_idx + 1; // Convert to 1-based tab index
                            switch_tab_to(tab_number as u32);
                        }
                    }
                }
                Mouse::ScrollUp(_) => {
                    switch_tab_to(min(self.active_tab_idx + 1, self.tabs.len()) as u32);
                }
                Mouse::ScrollDown(_) => {
                    switch_tab_to(max(self.active_tab_idx.saturating_sub(1), 1) as u32);
                }
                _ => {}
            },
            Event::PermissionRequestResult(result) => match result {
                PermissionStatus::Granted => set_selectable(false),
                PermissionStatus::Denied => eprintln!("Permission denied by user."),
            },
            _ => {
                eprintln!("Got unrecognized event: {:?}", event);
            }
        };
        should_render
    }

    fn pipe(&mut self, pipe_message: PipeMessage) -> bool {
        let mut should_render = false;
        match pipe_message.source {
            PipeSource::Keybind => {
                // Handle keybinding messages
                if pipe_message.name == "toggle_collapse" {
                    self.collapsed = !self.collapsed;
                    should_render = true;
                    
                    // Write state to filesystem for resize script
                    let state_file = "/tmp/.zj-sidebar-collapsed";
                    if self.collapsed {
                        std::fs::write(state_file, "1").ok();
                    } else {
                        std::fs::remove_file(state_file).ok();
                    }
                    
                    // Broadcast collapse state to all plugin instances
                    pipe_message_to_plugin(
                        MessageToPlugin {
                            plugin_url: None,
                            plugin_config: BTreeMap::new(),
                            message_name: "sync_collapse_state".to_string(),
                            message_payload: Some(self.collapsed.to_string()),
                            message_args: BTreeMap::new(),
                            new_plugin_args: None,
                            destination_plugin_id: None,
                        }
                    );
                }
            }
            PipeSource::Cli(_) => {
                if pipe_message.name == "zj-status-sidebar:cli:tab_alert" {
                    if let (Some(pane_id_str), Some(exit_code_str)) = (
                        pipe_message.args.get("pane_id"),
                        pipe_message.args.get("exit_code"),
                    ) {
                        let pane_id: u32 = match pane_id_str.parse() {
                            Ok(int) => int,
                            Err(..) => return false,
                        };
                        let exit_code: u32 = match exit_code_str.parse() {
                            Ok(int) => int,
                            Err(..) => return false,
                        };

                        for (tab_idx, pane_vec) in &self.pane_info.panes {
                            // skip panes in current tab
                            if *tab_idx == self.active_tab_idx - 1 {
                                continue;
                            }

                            // find index of tab containing the pane
                            if pane_vec.iter().any(|p| p.id == pane_id) {
                                let first_alert = self.tab_alerts.is_empty();

                                self.tab_alerts.insert(
                                    *tab_idx,
                                    TabAlert {
                                        success: exit_code == 0,
                                        alternate_color: true,
                                    },
                                );

                                // Only fire timer/re-render on the first alert, when the 1st timer
                                // expires the state is updated there and new timer is set.
                                if first_alert {
                                    set_timeout(1.0);
                                    should_render = true;
                                }

                                // tab index found, exit loop
                                break;
                            }
                        }
                    }
                }
            }
            PipeSource::Plugin(_source_plugin_id) => {
                // Handle plugin-to-plugin messages
                if pipe_message.name == "sync_collapse_state" {
                    if let Some(collapsed_str) = &pipe_message.payload {
                        if let Ok(new_collapsed) = collapsed_str.parse::<bool>() {
                            self.collapsed = new_collapsed;
                            should_render = true;
                            
                            // Update state file
                            let state_file = "/tmp/.zj-sidebar-collapsed";
                            if self.collapsed {
                                std::fs::write(state_file, "1").ok();
                            } else {
                                std::fs::remove_file(state_file).ok();
                            }
                        }
                    }
                } else if pipe_message.is_private
                    && pipe_message.name == "zj-status-sidebar:plugin:tab_alert:broadcast"
                    && self.tab_alerts.is_empty()
                {
                    self.tab_alerts = serde_json::from_str(&pipe_message.payload.unwrap()).unwrap();

                    // fire 1st timer/re-render
                    set_timeout(1.0);
                    should_render = true;
                }
            }
            _ => {
                should_render = false;
            }
        }
        should_render
    }

    fn render(&mut self, rows: usize, cols: usize) {
        if self.tabs.is_empty() {
            return;
        }
        
        // Store rows for mouse click handling
        self.rows = rows;
        
        // Clear tab_line for mouse click detection
        self.tab_line.clear();
        
        let background = match self.mode_info.style.colors.theme_hue {
            ThemeHue::Dark => self.mode_info.style.colors.black,
            ThemeHue::Light => self.mode_info.style.colors.white,
        };
        
        let text_color = match self.mode_info.style.colors.theme_hue {
            ThemeHue::Dark => self.mode_info.style.colors.white,
            ThemeHue::Light => self.mode_info.style.colors.black,
        };
        
        // Use ANSI escape codes to ensure proper multi-line rendering
        // First, clear the screen
        print!("\x1b[2J");
        
        // Row 1: Title with toggle button
        print!("\x1b[1;1H"); // Move to row 1, column 1
        let toggle_icon = if self.collapsed { "▶" } else { "◀" }; // ▶ or ◀
        let title = if self.collapsed { 
            if cols >= 6 {
                format!(" {} 📌 ", toggle_icon)
            } else {
                format!("{} 📌", toggle_icon)
            }
        } else { 
            format!("{} SIDEBAR V3", toggle_icon)
        };
        let title_line = style!(text_color, background)
            .bold()
            .paint(format!("{:^width$}", title, width = cols));
        print!("{}", title_line);
        
        // Row 2: Empty spacer
        print!("\x1b[2;1H"); // Move to row 2, column 1
        let empty_line = style!(text_color, background)
            .paint(format!("{:width$}", "", width = cols));
        print!("{}", empty_line);
        
        // All tabs are 3 lines tall
        let tab_height = 3;
        
        // Rows 3+: Tabs
        let mut current_row = 3;
        for (idx, t) in self.tabs.iter().enumerate() {
            if current_row + tab_height - 1 > rows {
                break;
            }
            
            // Get emoji for this tab (always use from cache)
            let generated_name = self.name_cache.get_or_generate(t.position).to_string();
            let emoji = generated_name.split_whitespace().next().unwrap_or("📄").to_string();
            
            // Use custom name if available, otherwise use generated name
            let display_name = if !t.name.is_empty() && !t.name.starts_with("Tab ") {
                format!("{} {}", emoji, t.name)
            } else {
                generated_name.clone()
            };
            
            let (fg_color, bg_color) = if t.active {
                (background, text_color)
            } else {
                (text_color, background)
            };
            
            // Check for alerts
            let alert_info = self.tab_alerts.get(&t.position);
            let (final_fg, final_bg) = if let Some(alert) = alert_info {
                let alert_color = if alert.success {
                    self.mode_info.style.colors.green
                } else {
                    self.mode_info.style.colors.red
                };
                if alert.alternate_color {
                    (fg_color, alert_color)
                } else {
                    (alert_color, bg_color)
                }
            } else {
                (fg_color, bg_color)
            };
            
            // Render tab across multiple rows
            for row_offset in 0..tab_height {
                print!("\x1b[{};1H", current_row + row_offset);
                
                let content = if row_offset == 0 || row_offset == 2 {
                    // First and last rows: empty for spacing
                    String::from("")
                } else if row_offset == 1 {
                    // Middle row: content
                    if self.collapsed {
                        // Just show emoji when collapsed, centered if space allows
                        if cols >= 3 {
                            format!("{:^width$}", emoji, width = cols)
                        } else {
                            emoji.chars().next().unwrap_or(' ').to_string()
                        }
                    } else {
                        // Show full name
                        format!(" {}", display_name)
                    }
                } else {
                    String::from("")
                };
                
                let formatted_content = if cols <= 3 {
                    // Very narrow - just show first character or space
                    content.chars().next().unwrap_or(' ').to_string()
                } else if content.len() > cols {
                    // Truncate if needed
                    if cols > 4 {
                        let mut truncated = content[..cols - 4].to_string();
                        truncated.push_str("...");
                        truncated
                    } else {
                        content[..cols.min(content.len())].to_string()
                    }
                } else {
                    content
                };
                
                let tab_line = if t.active {
                    style!(final_fg, final_bg)
                        .bold()
                        .paint(format!("{:width$}", formatted_content, width = cols))
                } else {
                    style!(final_fg, final_bg)
                        .paint(format!("{:width$}", formatted_content, width = cols))
                };
                
                print!("{}", tab_line);
            }
            
            self.tab_line.push(LinePart {
                part: String::new(),
                len: cols,
                tab_index: Some(idx),
            });
            
            current_row += tab_height;
        }
        
        // Fill remaining rows with background
        while current_row <= rows {
            print!("\x1b[{};1H", current_row); // Move to current row
            let empty_line = style!(text_color, background)
                .paint(format!("{:width$}", "", width = cols));
            print!("{}", empty_line);
            current_row += 1;
        }
        
        // Ensure output is flushed
        use std::io::{self, Write};
        let _ = io::stdout().flush();
    }
}
