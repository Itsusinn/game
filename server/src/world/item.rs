use protocol::Coord;

#[derive(Debug, Clone)]
pub struct Item {
    pub id: u32,
    pub name: String,
    pub item_type: ItemType,
}

#[derive(Debug, Clone)]
pub enum ItemType {
    Weapon { damage: i32, attack_bonus: i32 },
    Armor { defense: i32 },
    Potion { heal: i32 },
    Scroll { effect: String },
    Food { nutrition: i32 },
}

#[derive(Debug, Clone)]
pub struct ItemStack {
    pub item: Item,
    pub quantity: u32,
}

#[derive(Debug, Clone)]
pub struct Inventory {
    pub items: Vec<ItemStack>,
    pub capacity: usize,
}

impl Inventory {
    pub fn new(capacity: usize) -> Self {
        Self {
            items: Vec::new(),
            capacity,
        }
    }

    pub fn add(&mut self, stack: ItemStack) -> bool {
        if self.items.len() >= self.capacity {
            return false;
        }
        // Stack with existing same-type item
        if let Some(existing) = self.items.iter_mut().find(|s| s.item.id == stack.item.id) {
            existing.quantity += stack.quantity;
        } else {
            self.items.push(stack);
        }
        true
    }

    pub fn remove(&mut self, item_id: u32, quantity: u32) -> bool {
        if let Some(pos) = self.items.iter().position(|s| s.item.id == item_id) {
            if self.items[pos].quantity <= quantity {
                self.items.remove(pos);
            } else {
                self.items[pos].quantity -= quantity;
            }
            true
        } else {
            false
        }
    }

    pub fn has(&self, item_id: u32) -> bool {
        self.items.iter().any(|s| s.item.id == item_id)
    }
}

#[derive(Debug, Clone)]
pub struct GroundItem {
    pub stack: ItemStack,
    pub pos: Coord,
}

pub struct ItemManager {
    ground_items: Vec<GroundItem>,
    next_id: u32,
}

impl ItemManager {
    pub fn new() -> Self {
        Self {
            ground_items: Vec::new(),
            next_id: 1,
        }
    }

    pub fn spawn_ground(&mut self, name: &str, item_type: ItemType, pos: Coord, quantity: u32) {
        let id = self.next_id;
        self.next_id += 1;
        self.ground_items.push(GroundItem {
            stack: ItemStack {
                item: Item {
                    id,
                    name: name.to_string(),
                    item_type,
                },
                quantity,
            },
            pos,
        });
    }

    pub fn items_at(&self, x: i32, y: i32) -> Vec<&GroundItem> {
        self.ground_items
            .iter()
            .filter(|gi| gi.pos.x == x && gi.pos.y == y)
            .collect()
    }

    pub fn remove_at(&mut self, x: i32, y: i32, item_id: u32) -> Option<ItemStack> {
        if let Some(pos) = self
            .ground_items
            .iter()
            .position(|gi| gi.pos.x == x && gi.pos.y == y && gi.stack.item.id == item_id)
        {
            Some(self.ground_items.remove(pos).stack)
        } else {
            None
        }
    }

    pub fn all_ground_items(&self) -> Vec<(Coord, protocol::ItemStack)> {
        self.ground_items
            .iter()
            .map(|gi| {
                (
                    gi.pos,
                    protocol::ItemStack {
                        item_id: gi.stack.item.id,
                        quantity: gi.stack.quantity,
                    },
                )
            })
            .collect()
    }
}

impl Default for ItemManager {
    fn default() -> Self {
        Self::new()
    }
}
