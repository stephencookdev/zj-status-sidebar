// Random name generator for tabs (emoji + adjective + noun)

use std::collections::HashMap;

// Arrays of emojis, adjectives, and nouns
const EMOJIS: &[&str] = &[
    "🌟", "🚀", "🎨", "🌈", "⚡", "🔥", "❄️", "🌸", "🍀", "🦄",
    "🐉", "🦋", "🐢", "🦊", "🐙", "🦜", "🌺", "🍄", "🌙", "☀️",
    "💎", "🏔️", "🌊", "🍃", "🎭", "🎪", "🎯", "🎲", "🔮", "💫",
];

const ADJECTIVES: &[&str] = &[
    "happy", "bright", "swift", "gentle", "mighty", "clever", "brave", "calm",
    "eager", "jolly", "keen", "lively", "merry", "proud", "quirky", "radiant",
    "serene", "vivid", "witty", "zesty", "cosmic", "mystic", "noble", "ornate",
    "plucky", "rustic", "sleek", "unique", "valiant", "whimsical",
];

const NOUNS: &[&str] = &[
    "fox", "star", "moon", "wave", "flame", "storm", "cloud", "river",
    "mountain", "forest", "ocean", "desert", "meadow", "canyon", "glacier", "aurora",
    "comet", "nebula", "phoenix", "dragon", "falcon", "leopard", "dolphin", "butterfly",
    "crystal", "prism", "beacon", "horizon", "cascade", "zenith",
];

pub fn generate_tab_name(tab_index: usize) -> String {
    // Use tab index as seed for consistent names
    let emoji_idx = (tab_index * 7) % EMOJIS.len();
    let adj_idx = (tab_index * 13) % ADJECTIVES.len();
    let noun_idx = (tab_index * 23) % NOUNS.len();
    
    format!("{} {} {}", 
        EMOJIS[emoji_idx], 
        ADJECTIVES[adj_idx], 
        NOUNS[noun_idx]
    )
}

// Cache generated names to ensure consistency
pub struct NameCache {
    names: HashMap<usize, String>,
}

impl NameCache {
    pub fn new() -> Self {
        Self {
            names: HashMap::new(),
        }
    }
    
    pub fn get_or_generate(&mut self, tab_index: usize) -> &str {
        self.names.entry(tab_index)
            .or_insert_with(|| generate_tab_name(tab_index))
    }
}