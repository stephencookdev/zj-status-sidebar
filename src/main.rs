mod line;
mod tab;
mod names;

use std::cmp::{max, min};
use std::collections::{BTreeMap, HashMap};

use unicode_width::{UnicodeWidthStr, UnicodeWidthChar};
use zellij_tile::prelude::*;
use zellij_tile_utils::style;

use serde::{Deserialize, Serialize};
use crate::names::NameCache;


#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
enum AlertType {
    CommandResult { success: bool },
    Notification,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct TabAlert {
    alert_type: AlertType,
    alternate_color: bool,
    flash_count: u8,  // For notifications, counts down from 5
    persistent: bool, // For notifications, stays until tab is opened
}

impl Default for TabAlert {
    fn default() -> Self {
        Self {
            alert_type: AlertType::CommandResult { success: true },
            alternate_color: false,
            flash_count: 0,
            persistent: false,
        }
    }
}


struct State {
    pane_info: PaneManifest,
    tab_alerts: HashMap<usize, TabAlert>,
    tabs: Vec<TabInfo>,
    active_tab_idx: usize,
    mode_info: ModeInfo,
    name_cache: NameCache,
    rows: usize,
    cols: usize,
}

impl Default for State {
    fn default() -> Self {
        Self {
            pane_info: PaneManifest::default(),
            tab_alerts: HashMap::new(),
            tabs: Vec::new(),
            active_tab_idx: 0,
            mode_info: ModeInfo::default(),
            name_cache: NameCache::new(),
            rows: 0,
            cols: 0,
        }
    }
}


register_plugin!(State);

impl State {}

impl ZellijPlugin for State {
    fn load(&mut self, configuration: BTreeMap<String, String>) {
        eprintln!("[zj-status-sidebar] Plugin instance loading at {:?}", std::time::SystemTime::now());
        eprintln!("[zj-status-sidebar] Config: {:?}", configuration);
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
        
        // Set session seed if we have session name
        if let Some(ref session_name) = self.mode_info.session_name {
            self.name_cache.set_session_seed(session_name);
        }
        
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
                    
                    // Set session seed when we get session name
                    if let Some(ref session_name) = mode_info.session_name {
                        self.name_cache.set_session_seed(session_name);
                    }
                }
                self.mode_info = mode_info
            }
            Event::Timer(_) => {
                // Handle tab alerts and notifications
                if !self.tab_alerts.is_empty() {
                    let mut alerts_to_remove = Vec::new();
                    
                    for (tab_idx, tab_alert) in self.tab_alerts.iter_mut() {
                        match &tab_alert.alert_type {
                            AlertType::CommandResult { .. } => {
                                // Toggle color for command results
                                tab_alert.alternate_color = !tab_alert.alternate_color;
                            }
                            AlertType::Notification => {
                                // Handle notification flashing
                                if tab_alert.flash_count > 0 {
                                    tab_alert.alternate_color = !tab_alert.alternate_color;
                                    if tab_alert.alternate_color {
                                        // Only decrement on the "off" phase of flash
                                        tab_alert.flash_count -= 1;
                                    }
                                } else if !tab_alert.persistent {
                                    // Remove non-persistent notifications after flashing
                                    alerts_to_remove.push(*tab_idx);
                                }
                            }
                        }
                    }
                    
                    // Remove finished alerts
                    for idx in alerts_to_remove {
                        self.tab_alerts.remove(&idx);
                    }
                    
                    should_render = true;
                    set_timeout(1.0); // Continue timer for alerts
                }
            }
            Event::TabUpdate(tabs) => {
                if let Some(active_tab_index) = tabs.iter().position(|t| t.active) {
                    let active_tab_idx = active_tab_index + 1;
                    let tab_changed = self.active_tab_idx != active_tab_idx;
                    
                    if tab_changed || self.tabs != tabs {
                        // Remove alerts when tab becomes active (using position, not index)
                        if active_tab_index < tabs.len() {
                            let active_tab_position = tabs[active_tab_index].position;
                            self.tab_alerts.remove(&active_tab_position);
                        }
                        should_render = true;
                    }
                    self.active_tab_idx = active_tab_idx;
                    self.tabs = tabs;
                } else {
                    eprintln!("Could not find active tab.");
                }
            }
            Event::Key(key) => {
                if self.mode_info.mode == InputMode::Tab {
                    match key {
                        KeyWithModifier { bare_key: BareKey::Char('t'), .. } => {
                            // Don't render here - let Zellij handle the mode switch
                            // The ModeUpdate event will trigger a render if needed
                            switch_to_input_mode(&InputMode::Normal);
                        }
                        KeyWithModifier { bare_key: BareKey::Char('r'), .. } => {
                            // User pressed 'r' to rename tab - trigger a render to show rename UI
                            should_render = true;
                        }
                        _ => {}
                    }
                } else if self.mode_info.mode == InputMode::RenameTab {
                    // We're in rename mode - always render to show the input
                    should_render = true;
                }
            }
            Event::Mouse(me) => match me {
                Mouse::LeftClick(row, _col) => {
                    if row >= 2 {
                        let tab_height = 3;
                        let tab_idx = (row as usize - 2) / tab_height;
                        if tab_idx < self.tabs.len() {
                            let tab_number = tab_idx + 1;
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
            _ => {}
        };
        should_render
    }

    fn pipe(&mut self, pipe_message: PipeMessage) -> bool {
        let mut should_render = false;
        match pipe_message.source {
            PipeSource::Keybind => {
                if pipe_message.name == "toggle_collapse" {
                    eprintln!("[zj-status-sidebar] Toggle keybind pressed (Ctrl+t,t) - feature temporarily disabled");
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
                            if *tab_idx == self.active_tab_idx - 1 {
                                continue;
                            }

                            if pane_vec.iter().any(|p| p.id == pane_id) {
                                let first_alert = self.tab_alerts.is_empty();

                                self.tab_alerts.insert(
                                    *tab_idx,
                                    TabAlert {
                                        alert_type: AlertType::CommandResult { success: exit_code == 0 },
                                        alternate_color: true,
                                        flash_count: 0,
                                        persistent: false,
                                    },
                                );

                                if first_alert {
                                    set_timeout(1.0);
                                    should_render = true;
                                }

                                break;
                            }
                        }
                    }
                } else if pipe_message.name == "zj-status-sidebar:cli:notify" {
                    // Handle notification request
                    // Usage: zellij pipe --name "zj-status-sidebar:cli:notify" --args "tab=1"
                    //    or: zellij pipe --name "zj-status-sidebar:cli:notify" --args "tab_name=main"
                    
                    let tab_idx = if let Some(tab_str) = pipe_message.args.get("tab") {
                        // Tab specified by index (1-based)
                        match tab_str.parse::<usize>() {
                            Ok(idx) if idx > 0 => Some(idx - 1),
                            _ => None,
                        }
                    } else if let Some(tab_name) = pipe_message.args.get("tab_name") {
                        // Tab specified by name
                        self.tabs.iter().position(|t| t.name == *tab_name)
                    } else {
                        None
                    };
                    
                    if let Some(idx) = tab_idx {
                        // Get the tab position for the tab alerts map
                        if idx < self.tabs.len() {
                            let tab_position = self.tabs[idx].position;
                            
                            // Don't notify the active tab
                            if idx != self.active_tab_idx - 1 {
                                let first_alert = self.tab_alerts.is_empty();
                                
                                self.tab_alerts.insert(
                                    tab_position,
                                    TabAlert {
                                        alert_type: AlertType::Notification,
                                        alternate_color: false,
                                        flash_count: 10,  // 5 full flashes (on/off = 2 states)
                                        persistent: true,
                                    },
                                );
                                
                                if first_alert {
                                    set_timeout(0.2);  // Faster timer for flashing
                                }
                                should_render = true;
                                
                                eprintln!("[zj-status-sidebar] Notification sent to tab {} (position {})", idx + 1, tab_position);
                            }
                        } else {
                            eprintln!("[zj-status-sidebar] Tab index out of range");
                        }
                    } else {
                        eprintln!("[zj-status-sidebar] Invalid tab specified for notification");
                    }
                }
            }
            PipeSource::Plugin(_source_plugin_id) => {
                if pipe_message.is_private
                    && pipe_message.name == "zj-status-sidebar:plugin:tab_alert:broadcast"
                {
                    if self.tab_alerts.is_empty() {
                        if let Some(payload) = &pipe_message.payload {
                            if let Ok(new_alerts) = serde_json::from_str::<HashMap<usize, TabAlert>>(payload) {
                                if self.tab_alerts != new_alerts {
                                    self.tab_alerts = new_alerts;
                                    set_timeout(1.0);
                                    should_render = true;
                                }
                            }
                        }
                    }
                }
            }
        }
        should_render
    }

    fn render(&mut self, rows: usize, cols: usize) {
        if self.tabs.is_empty() || rows == 0 || cols == 0 {
            return;
        }
        
        // Update dimensions
        self.cols = cols;
        self.rows = rows;
        
        let background = self.mode_info.style.colors.ribbon_unselected.background;
        let text_color = self.mode_info.style.colors.ribbon_unselected.base;
        
        print!("\x1b[2J");
        
        // Row 1: Title
        print!("\x1b[1;1H");
        let title = "SIDEBAR V3";
        let title_line = style!(text_color, background)
            .bold()
            .paint(format!("{:^width$}", title, width = cols));
        print!("{}", title_line);
        
        // Row 2: Spacer
        print!("\x1b[2;1H");
        let empty_line = style!(text_color, background)
            .paint(format!("{:width$}", "", width = cols));
        print!("{}", empty_line);
        
        // Tabs
        let tab_height = 3;
        let mut current_row = 3;
        
        for (_idx, t) in self.tabs.iter().enumerate() {
            if current_row + tab_height - 1 > rows {
                break;
            }
            
            let generated_name = self.name_cache.get_or_generate(t.position).to_string();
            let emoji = generated_name.split_whitespace().next().unwrap_or("📄").to_string();
            
            let display_name = if !t.name.is_empty() && !t.name.starts_with("Tab ") {
                format!("{} {}", emoji, t.name)
            } else {
                generated_name.clone()
            };
            
            // Show rename indicator if this is the active tab and we're in rename mode
            let is_renaming = t.active && self.mode_info.mode == InputMode::RenameTab;
            let mut display_name_with_indicator = if is_renaming {
                // Replace the emoji with pencil, keep the rest of the name
                let name_parts: Vec<&str> = display_name.splitn(2, ' ').collect();
                if name_parts.len() > 1 {
                    format!("✏️ {}", name_parts[1])
                } else {
                    format!("✏️ {}", display_name)
                }
            } else {
                display_name
            };
            
            let (fg_color, bg_color) = if t.active {
                (background, text_color)
            } else {
                (text_color, background)
            };
            
            let alert_info = self.tab_alerts.get(&t.position);
            let (final_fg, final_bg, notification_indicator) = if let Some(alert) = alert_info {
                match &alert.alert_type {
                    AlertType::CommandResult { success } => {
                        let alert_color = if *success {
                            self.mode_info.style.colors.frame_highlight.background
                        } else {
                            self.mode_info.style.colors.frame_unselected.unwrap_or_default().background
                        };
                        let (fg, bg) = if alert.alternate_color {
                            (fg_color, alert_color)
                        } else {
                            (alert_color, bg_color)
                        };
                        (fg, bg, None)
                    }
                    AlertType::Notification => {
                        // Red color for notifications
                        let red_color = self.mode_info.style.colors.frame_unselected.unwrap_or_default().background;
                        let (fg, bg) = if alert.alternate_color || alert.flash_count == 0 {
                            (fg_color, red_color)
                        } else {
                            (fg_color, bg_color)
                        };
                        (fg, bg, Some("🔴"))
                    }
                }
            } else {
                (fg_color, bg_color, None)
            };
            
            // Add notification indicator to the display name
            if let Some(indicator) = notification_indicator {
                // Prepend the indicator
                display_name_with_indicator = format!("{} {}", indicator, display_name_with_indicator);
            }
            
            for row_offset in 0..tab_height {
                print!("\x1b[{};1H", current_row + row_offset);
                
                let content = if row_offset == 0 || row_offset == 2 {
                    String::from("")
                } else if row_offset == 1 {
                    // Display emoji + name with left padding
                    format!(" {}", display_name_with_indicator)
                } else {
                    String::from("")
                };
                
                let formatted_content = safe_truncate_to_width(&content, cols);
                
                let tab_line = if t.active {
                    style!(final_fg, final_bg)
                        .bold()
                        .paint(&formatted_content)
                } else {
                    style!(final_fg, final_bg)
                        .paint(&formatted_content)
                };
                
                print!("{}", tab_line);
            }
            
            current_row += tab_height;
        }
        
        // Fill remaining
        while current_row <= rows {
            print!("\x1b[{};1H", current_row);
            let empty_line = style!(text_color, background)
                .paint(format!("{:width$}", "", width = cols));
            print!("{}", empty_line);
            current_row += 1;
        }
        
        use std::io::{self, Write};
        let _ = io::stdout().flush();
    }
}

fn safe_truncate_to_width(s: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }
    
    let display_width = s.width();
    
    if display_width <= max_width {
        // For single emojis or very short strings, always center them
        if display_width <= 2 {
            // Center the emoji
            let padding_total = max_width.saturating_sub(display_width);
            let padding_left = padding_total / 2;
            let padding_right = padding_total - padding_left;
            
            // Debug log for centering
            if s.chars().any(|c| c > '\u{1F000}') {  // Likely an emoji
                eprintln!("[zj-status-sidebar] Centering '{}': width={}, max_width={}, pad_left={}, pad_right={}", 
                         s, display_width, max_width, padding_left, padding_right);
            }
            
            let mut result = " ".repeat(padding_left);
            result.push_str(s);
            result.push_str(&" ".repeat(padding_right));
            return result;
        }
        return format!("{:width$}", s, width = max_width);
    }
    
    if max_width <= 3 {
        let mut result = String::new();
        let mut current_width = 0;
        
        for ch in s.chars() {
            let ch_width = ch.width().unwrap_or(0);
            if current_width + ch_width <= max_width {
                result.push(ch);
                current_width += ch_width;
            } else {
                break;
            }
        }
        
        while current_width < max_width {
            result.push(' ');
            current_width += 1;
        }
        
        return result;
    }
    
    let target_width = max_width.saturating_sub(3);
    let mut result = String::new();
    let mut current_width = 0;
    
    for ch in s.chars() {
        let ch_width = ch.width().unwrap_or(0);
        if current_width + ch_width <= target_width {
            result.push(ch);
            current_width += ch_width;
        } else {
            break;
        }
    }
    
    result.push_str("...");
    
    let final_width = result.width();
    if final_width < max_width {
        for _ in final_width..max_width {
            result.push(' ');
        }
    }
    
    result
}