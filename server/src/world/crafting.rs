use crate::world::item::{ItemManager, ItemType};

#[derive(Debug, Clone)]
pub struct Recipe {
    pub name: String,
    pub ingredients: Vec<(String, u32)>,
    pub result: (String, ItemType, u32),
}

pub struct CraftingManager {
    recipes: Vec<Recipe>,
}

impl CraftingManager {
    pub fn new() -> Self {
        let recipes = vec![
            Recipe {
                name: "Bandage".into(),
                ingredients: vec![("Cloth".into(), 2)],
                result: ("Bandage".into(), ItemType::Potion { heal: 10 }, 1),
            },
            Recipe {
                name: "Torch".into(),
                ingredients: vec![("Stick".into(), 1), ("Cloth".into(), 1)],
                result: ("Torch".into(), ItemType::Scroll { effect: "light".into() }, 1),
            },
            Recipe {
                name: "Improvised Weapon".into(),
                ingredients: vec![("Stick".into(), 2), ("Stone".into(), 1)],
                result: ("Club".into(), ItemType::Weapon { damage: 4, attack_bonus: 0 }, 1),
            },
            Recipe {
                name: "Trail Mix".into(),
                ingredients: vec![("Berries".into(), 2), ("Nuts".into(), 1)],
                result: ("Trail Mix".into(), ItemType::Food { nutrition: 40 }, 1),
            },
        ];

        Self { recipes }
    }

    pub fn get_recipe(&self, name: &str) -> Option<&Recipe> {
        self.recipes.iter().find(|r| r.name == name)
    }

    pub fn all_recipes(&self) -> &[Recipe] {
        &self.recipes
    }

    pub fn can_craft(&self, name: &str, available_items: &[(String, u32)]) -> bool {
        if let Some(recipe) = self.get_recipe(name) {
            recipe.ingredients.iter().all(|(ing_name, ing_qty)| {
                available_items
                    .iter()
                    .any(|(an, aq)| an == ing_name && aq >= ing_qty)
            })
        } else {
            false
        }
    }
}

impl Default for CraftingManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ItemManager {
    /// Craft an item if possible. Returns the crafted item name, or None.
    pub fn try_craft(
        &mut self,
        recipe_name: &str,
        available_items: &mut Vec<(String, u32)>,
        x: i32,
        y: i32,
        crafting: &CraftingManager,
    ) -> Option<String> {
        let recipe = crafting.get_recipe(recipe_name)?;

        // Check ingredients
        if !crafting.can_craft(recipe_name, available_items) {
            return None;
        }

        // Consume ingredients
        for (ing_name, ing_qty) in &recipe.ingredients {
            if let Some(item) = available_items.iter_mut().find(|(n, _)| n == ing_name) {
                item.1 = item.1.saturating_sub(*ing_qty);
            }
        }
        available_items.retain(|(_, q)| *q > 0);

        // Spawn result
        let (name, item_type, quantity) = &recipe.result;
        self.spawn_ground(name, item_type.clone(), protocol::Coord::new(x, y), *quantity);
        Some(name.clone())
    }
}
