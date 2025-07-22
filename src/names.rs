// Random name generator for tabs (emoji + adjective + noun)

use std::collections::HashMap;

// Arrays of emojis, adjectives, and nouns
const EMOJIS: &[&str] = &[
    "🌟", "🚀", "🎨", "🌈", "⚡", "🔥", "❄️", "🌸", "🍀", "🦄",
    "🐉", "🦋", "🐢", "🦊", "🐙", "🦜", "🌺", "🍄", "🌙", "☀️",
    "💎", "🏔️", "🌊", "🍃", "🎭", "🎪", "🎯", "🎲", "🔮", "💫",
    "🎸", "🎹", "🎺", "🎷", "🥁", "🎵", "🎶", "🎼", "🎤", "🎧",
    "📚", "📖", "💡", "🔍", "🔬", "🔭", "🔧", "⚙️", "🗝️", "🛡️",
    "🌱", "🌿", "🍁", "🍂", "🌾", "🌵", "🌴", "🌲", "🌳", "🌷",
    "🏖️", "🏝️", "🏜️", "🏞️", "🗻", "🌋", "🏛️", "🏰", "🗼", "🌉",
    "🦁", "🐯", "🐨", "🐼", "🦘", "🦓", "🦒", "🦌", "🦚", "🦩",
    "🍎", "🍊", "🍋", "🍓", "🍇", "🍉", "🥝", "🍑", "🍒", "🥭",
    "⭐", "✨", "🌠", "☄️", "🌌", "🪐", "🛸", "🚁", "✈️", "🛩️",
];

const ADJECTIVES: &[&str] = &[
    "happy", "bright", "swift", "gentle", "mighty", "clever", "brave", "calm",
    "eager", "jolly", "keen", "lively", "merry", "proud", "quirky", "radiant",
    "serene", "vivid", "witty", "zesty", "cosmic", "mystic", "noble", "ornate",
    "plucky", "rustic", "sleek", "unique", "valiant", "whimsical", "agile", "bold",
    "crisp", "daring", "elegant", "fierce", "graceful", "humble", "intense", "jovial",
    "kindly", "luminous", "majestic", "nimble", "peaceful", "quick", "royal", "spirited",
    "tranquil", "upbeat", "vibrant", "wise", "zealous", "artistic", "bouncy", "charming",
    "dreamy", "ethereal", "friendly", "gleaming", "heroic", "inspired", "joyful", "kinetic",
];

const NOUNS: &[&str] = &[
    "fox", "star", "moon", "wave", "flame", "storm", "cloud", "river",
    "mountain", "forest", "ocean", "desert", "meadow", "canyon", "glacier", "aurora",
    "comet", "nebula", "phoenix", "dragon", "falcon", "leopard", "dolphin", "butterfly",
    "crystal", "prism", "beacon", "horizon", "cascade", "zenith", "adventure", "breeze",
    "cosmos", "dream", "echo", "fountain", "garden", "harmony", "island", "journey",
    "kaleidoscope", "lighthouse", "melody", "nova", "oasis", "paradise", "quest", "rainbow",
    "sanctuary", "twilight", "universe", "valley", "whisper", "zephyr", "arbor", "bloom",
    "citadel", "dawn", "ember", "frost", "glow", "haven", "iris", "jewel",
];

// Simple hash function to generate deterministic random-looking numbers
fn simple_hash(seed: u64) -> u64 {
    let mut x = seed;
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
    x ^ (x >> 31)
}

pub fn generate_tab_name(tab_index: usize, session_seed: u64) -> String {
    // For unique emojis, use tab_index modulo emoji count
    // This ensures each tab gets a unique emoji up to the number of available emojis
    let emoji_idx = tab_index % EMOJIS.len();
    
    // Mix the tab index with session seed for random adjective/noun
    let seed = simple_hash(session_seed.wrapping_add(tab_index as u64));
    let adj_idx = (simple_hash(seed) % ADJECTIVES.len() as u64) as usize;
    let noun_idx = (simple_hash(simple_hash(seed)) % NOUNS.len() as u64) as usize;
    
    format!("{} {} {}", 
        EMOJIS[emoji_idx], 
        ADJECTIVES[adj_idx], 
        NOUNS[noun_idx]
    )
}

// Cache generated names to ensure consistency
pub struct NameCache {
    names: HashMap<usize, String>,
    session_seed: u64,
}

impl NameCache {
    pub fn new() -> Self {
        Self {
            names: HashMap::new(),
            session_seed: 0,  // Will be set based on session name
        }
    }
    
    pub fn set_session_seed(&mut self, session_name: &str) {
        // Generate deterministic seed from session name
        // This ensures all plugin instances in the same session use the same seed
        let mut seed = 0u64;
        for (i, byte) in session_name.bytes().enumerate() {
            seed = seed.wrapping_add((byte as u64).wrapping_mul((i + 1) as u64));
            seed = simple_hash(seed);
        }
        self.session_seed = seed;
        // Clear any cached names to regenerate with new seed
        self.names.clear();
    }
    
    pub fn get_or_generate(&mut self, tab_index: usize) -> &str {
        self.names.entry(tab_index)
            .or_insert_with(|| generate_tab_name(tab_index, self.session_seed))
    }
}