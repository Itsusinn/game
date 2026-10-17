use serde::{Deserialize, Serialize};

/// Inventory display stacks use a world-independent item definition name.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InventoryEntry {
    pub name: String,
    pub quantity: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LogEntry {
    pub text: String,
    pub color: u32,
    pub turn: u64,
}
