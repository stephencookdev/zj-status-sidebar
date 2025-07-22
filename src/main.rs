mod line;
mod tab;
mod names;

use std::cmp::{max, min};
use std::collections::{BTreeMap, HashMap};
use std::time::{SystemTime, UNIX_EPOCH};

use unicode_width::{UnicodeWidthStr, UnicodeWidthChar};
use zellij_tile::prelude::*;
use zellij_tile::shim::next_swap_layout;
use zellij_tile_utils::style;

use serde::{Deserialize, Serialize};
use crate::names::NameCache;


#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
struct TabAlert {
    success: bool,
    alternate_color: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct StateEntry {
    timestamp: u64,  // Unix timestamp in milliseconds
    collapsed: bool,
}

impl StateEntry {
    fn new(collapsed: bool) -> Self {
        Self {
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            collapsed,
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
    // Single most recent state
    collapsed_state: Option<StateEntry>,
    last_file_mtime: Option<SystemTime>,
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
            collapsed_state: None,
            last_file_mtime: None,
        }
    }
}


register_plugin!(State);

impl State {
    // Get the current desired state from local memory
    fn get_desired_collapsed(&self) -> bool {
        self.collapsed_state
            .as_ref()
            .map(|entry| entry.collapsed)
            .unwrap_or(false)  // Default to expanded
    }
    
    // Add a new state to local memory
    fn add_state(&mut self, collapsed: bool) {
        let entry = StateEntry::new(collapsed);
        self.collapsed_state = Some(entry.clone());
        
        // Write to file atomically
        self.write_state_file(&entry);
    }
    
    // Write state to file
    fn write_state_file(&self, entry: &StateEntry) {
        let state_file = "/tmp/.zj-sidebar-state.json";
        let temp_file = "/tmp/.zj-sidebar-state.json.tmp";
        
        if let Ok(json) = serde_json::to_string(entry) {
            // Write to temp file first
            if std::fs::write(temp_file, json).is_ok() {
                // Atomic rename on Unix
                let _ = std::fs::rename(temp_file, state_file);
            }
        }
    }
    
    // Read state from file
    fn read_state_file(&self) -> Option<StateEntry> {
        let state_file = "/tmp/.zj-sidebar-state.json";
        std::fs::read_to_string(state_file)
            .ok()
            .and_then(|content| serde_json::from_str(&content).ok())
    }
    
    // Check if file has been modified since last check
    fn file_modified_since_last_check(&self) -> bool {
        let state_file = "/tmp/.zj-sidebar-state.json";
        
        // Get file metadata
        let Ok(metadata) = std::fs::metadata(state_file) else {
            return false;
        };
        
        // Get file modification time
        let Ok(mtime) = metadata.modified() else {
            return false;
        };
        
        // Check if newer than our last known mtime
        match self.last_file_mtime {
            Some(last_mtime) => mtime > last_mtime,
            None => true,  // First check
        }
    }
    
    // Update state from file if newer
    fn update_from_file(&mut self) {
        // Only read file if it has been modified
        if !self.file_modified_since_last_check() {
            return;
        }
        
        let state_file = "/tmp/.zj-sidebar-state.json";
        
        // Update our last known mtime
        if let Ok(metadata) = std::fs::metadata(state_file) {
            if let Ok(mtime) = metadata.modified() {
                self.last_file_mtime = Some(mtime);
            }
        }
        
        // Read and parse the file
        if let Some(file_entry) = self.read_state_file() {
            // Check if file has newer state than our current
            let should_update = match &self.collapsed_state {
                Some(current) => file_entry.timestamp > current.timestamp,
                None => true,  // No local state yet
            };
            
            if should_update {
                self.collapsed_state = Some(file_entry);
            }
        }
    }
}

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
        
        // Load initial state from file
        self.update_from_file();
        
        // Start timer for periodic checks
        set_timeout(0.5);
        
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
                // Update from file periodically
                let old_state = self.get_desired_collapsed();
                self.update_from_file();
                let new_state = self.get_desired_collapsed();
                
                // If state changed, trigger layout update
                if old_state != new_state {
                    next_swap_layout();
                    should_render = true;
                }
                
                // Handle tab alerts
                if !self.tab_alerts.is_empty() {
                    for tab_alert in self.tab_alerts.values_mut() {
                        *tab_alert = TabAlert {
                            success: tab_alert.success,
                            alternate_color: !tab_alert.alternate_color,
                        }
                    }
                    should_render = true;
                }
                
                // Keep timer running
                set_timeout(0.5);
            }
            Event::TabUpdate(tabs) => {
                if let Some(active_tab_index) = tabs.iter().position(|t| t.active) {
                    let active_tab_idx = active_tab_index + 1;
                    let tab_changed = self.active_tab_idx != active_tab_idx;
                    
                    if tab_changed || self.tabs != tabs {
                        self.tab_alerts.remove(&active_tab_index);
                        should_render = true;
                        
                        // When tab opens, check file
                        if tab_changed {
                            self.update_from_file();
                        }
                    }
                    self.active_tab_idx = active_tab_idx;
                    self.tabs = tabs;
                } else {
                    eprintln!("Could not find active tab.");
                }
            }
            Event::Key(key) => {
                if self.mode_info.mode == InputMode::Tab {
                    if let KeyWithModifier { bare_key: BareKey::Char('t'), .. } = key {
                        should_render = true;
                        switch_to_input_mode(&InputMode::Normal);
                    }
                }
            }
            Event::Mouse(me) => match me {
                Mouse::LeftClick(row, _col) => {
                    if row == 0 {
                        // Toggle: get current state and flip it
                        let current = self.get_desired_collapsed();
                        let new_state = !current;
                        
                        // Update local memory first
                        self.add_state(new_state);
                        
                        // Trigger layout change
                        next_swap_layout();
                        
                        should_render = true;
                    } else if row >= 2 {
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
                    // Toggle: get current state and flip it
                    let current = self.get_desired_collapsed();
                    let new_state = !current;
                    
                    // Update local memory first
                    self.add_state(new_state);
                    
                    // Trigger layout change
                    next_swap_layout();
                    
                    should_render = true;
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
                                        success: exit_code == 0,
                                        alternate_color: true,
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
        
        self.cols = cols;
        self.rows = rows;
        
        // Local memory controls BOTH text display AND pane width
        let desired_collapsed = self.get_desired_collapsed();
        
        // Determine if we're actually collapsed based on column width
        let is_visually_collapsed = self.cols <= 10;
        
        // If mismatch between desired and actual, keep trying to fix it
        if desired_collapsed != is_visually_collapsed {
            next_swap_layout();
        }
        
        let background = self.mode_info.style.colors.ribbon_unselected.background;
        let text_color = self.mode_info.style.colors.ribbon_unselected.base;
        
        print!("\x1b[2J");
        
        // Row 1: Title
        print!("\x1b[1;1H");
        let toggle_icon = if desired_collapsed { "▶" } else { "◀" };
        let title = if desired_collapsed { 
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
            
            let (fg_color, bg_color) = if t.active {
                (background, text_color)
            } else {
                (text_color, background)
            };
            
            let alert_info = self.tab_alerts.get(&t.position);
            let (final_fg, final_bg) = if let Some(alert) = alert_info {
                let alert_color = if alert.success {
                    self.mode_info.style.colors.frame_highlight.background
                } else {
                    self.mode_info.style.colors.frame_unselected.unwrap_or_default().background
                };
                if alert.alternate_color {
                    (fg_color, alert_color)
                } else {
                    (alert_color, bg_color)
                }
            } else {
                (fg_color, bg_color)
            };
            
            for row_offset in 0..tab_height {
                print!("\x1b[{};1H", current_row + row_offset);
                
                let content = if row_offset == 0 || row_offset == 2 {
                    String::from("")
                } else if row_offset == 1 {
                    // Use desired state for display
                    if desired_collapsed {
                        if cols >= 3 {
                            format!("{:^width$}", emoji, width = cols)
                        } else {
                            emoji.chars().next().unwrap_or(' ').to_string()
                        }
                    } else {
                        format!(" {}", display_name)
                    }
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