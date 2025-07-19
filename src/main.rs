mod line;
mod tab;

use std::cmp::{max, min};
use std::collections::BTreeMap;
use std::collections::HashMap;

use zellij_tile::prelude::*;
use zellij_tile_utils::style;

use serde::{Deserialize, Serialize};

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

#[derive(Default)]
struct State {
    pane_info: PaneManifest,
    tab_alerts: HashMap<usize, TabAlert>,
    tabs: Vec<TabInfo>,
    active_tab_idx: usize,
    mode_info: ModeInfo,
    tab_line: Vec<LinePart>,
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
            EventType::PermissionRequestResult,
            EventType::Timer,
        ]);
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
            Event::Mouse(me) => match me {
                Mouse::LeftClick(row, _col) => {
                    // First three rows are header (empty, mode, separator), tabs start at row 3 (0-indexed)
                    if row >= 3 && (row as usize - 3) < self.tabs.len() {
                        let tab_index = (row as usize - 3) + 1; // Convert to 1-based tab index
                        switch_tab_to(tab_index as u32);
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
                // This message is sent by other plugin instances on each `Timer` event and
                // contains the state of tabs alerts.
                //
                // Only read it if the current instance doesn't contain any info (new tab created
                // after alerts were piped from a pane) to "catch up" and render them.
                if pipe_message.is_private
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
        
        // Row 1: Empty for now (could add title later)
        print!("\x1b[1;1H"); // Move to row 1, column 1
        let empty_line = style!(text_color, background)
            .paint(format!("{:width$}", "", width = cols));
        print!("{}", empty_line);
        
        // Row 2: Mode (NORMAL, LOCKED, etc)
        print!("\x1b[2;1H"); // Move to row 2, column 1
        let mode_text = format!("{:?}", self.mode_info.mode).to_uppercase();
        let mode_color = match self.mode_info.mode {
            InputMode::Locked => self.mode_info.style.colors.magenta,
            InputMode::Normal => self.mode_info.style.colors.green,
            _ => self.mode_info.style.colors.orange,
        };
        let mode_line = style!(text_color, mode_color)
            .bold()
            .paint(format!("{:^width$}", mode_text, width = cols));
        print!("{}", mode_line);
        
        // Row 3: Separator
        print!("\x1b[3;1H"); // Move to row 3, column 1
        let separator = "─".repeat(cols);
        let separator_line = style!(text_color, background)
            .paint(&separator);
        print!("{}", separator_line);
        
        // Rows 4+: Tabs
        let mut current_row = 4;
        for (idx, t) in self.tabs.iter().enumerate() {
            if current_row > rows {
                break;
            }
            
            print!("\x1b[{};1H", current_row); // Move to current row, column 1
            
            let mut tabname = format!("{} {}", t.position + 1, t.name);
            if tabname.len() > cols - 2 {
                tabname.truncate(cols - 5);
                tabname.push_str("...");
            }
            
            let (fg_color, bg_color) = if t.active {
                (background, text_color)
            } else {
                (text_color, background)
            };
            
            // Check for alerts
            let tab_line = if let Some(alert) = self.tab_alerts.get(&t.position) {
                let alert_color = if alert.success {
                    self.mode_info.style.colors.green
                } else {
                    self.mode_info.style.colors.red
                };
                
                if alert.alternate_color {
                    style!(fg_color, alert_color)
                        .bold()
                        .paint(format!(" {:width$}", tabname, width = cols - 1))
                } else {
                    style!(alert_color, bg_color)
                        .bold()
                        .paint(format!(" {:width$}", tabname, width = cols - 1))
                }
            } else if t.active {
                style!(fg_color, bg_color)
                    .bold()
                    .paint(format!(" {:width$}", tabname, width = cols - 1))
            } else {
                style!(fg_color, bg_color)
                    .italic()
                    .paint(format!(" {:width$}", tabname, width = cols - 1))
            };
            
            print!("{}", tab_line);
            
            self.tab_line.push(LinePart {
                part: String::new(),
                len: cols,
                tab_index: Some(idx),
            });
            
            current_row += 1;
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
