use crate::InventoryEntry;
use serde::{Deserialize, Serialize};

pub const HOTBAR_SLOTS: usize = 9;
pub const INVENTORY_SLOTS: usize = 36;
pub const ITEM_STACK_LIMIT: u32 = 64;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlayerInventory {
    /// Slots 0..9 are the hotbar; 9..36 are the main inventory.
    pub slots: Vec<Option<InventoryEntry>>,
    pub revision: u64,
    pub selected: u8,
}
impl Default for PlayerInventory {
    fn default() -> Self {
        Self {
            slots: vec![None; INVENTORY_SLOTS],
            revision: 0,
            selected: 0,
        }
    }
}
impl PlayerInventory {
    pub fn iter(&self) -> impl Iterator<Item = &InventoryEntry> {
        self.slots.iter().flatten()
    }
    pub fn len(&self) -> usize {
        self.iter().count()
    }
    pub fn is_empty(&self) -> bool {
        self.iter().next().is_none()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum InventoryCommand {
    Select {
        slot: u8,
    },
    Move {
        revision: u64,
        from: u8,
        to: u8,
        quantity: u32,
    },
    QuickMove {
        revision: u64,
        slot: u8,
    },
}
