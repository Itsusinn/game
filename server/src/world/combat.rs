use rand::Rng;

pub struct CombatResult {
    pub hit: bool,
    pub damage: i32,
    pub critical: bool,
    pub roll: i32,
}

pub fn melee_attack(attacker_atk: i32, defender_def: i32, base_damage: i32) -> CombatResult {
    let mut rng = rand::thread_rng();
    let roll = rng.gen_range(1..=20);
    resolve_attack(attacker_atk, defender_def, base_damage, roll)
}

fn resolve_attack(
    attacker_atk: i32,
    defender_def: i32,
    base_damage: i32,
    roll: i32,
) -> CombatResult {
    let critical = roll == 20;
    let hit = critical || roll + attacker_atk >= defender_def;

    let damage = if critical {
        base_damage * 2
    } else if hit {
        base_damage
    } else {
        0
    };

    CombatResult {
        hit,
        damage,
        critical,
        roll,
    }
}

/// Entity stat lookup — entities of different types have different stats
pub fn get_entity_stats(entity_type: u8) -> (i32, i32, i32) {
    // (atk, def, base_damage)
    match entity_type {
        0 => (3, 2, 5), // Player
        1 => (1, 1, 4), // Zombie
        2 => (2, 2, 5), // Skeleton
        3 => (2, 1, 3), // Goblin
        4 => (1, 0, 2), // Rat
        _ => (1, 1, 3),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_combat_hit() {
        for roll in 1..=20 {
            let result = resolve_attack(5, 2, 6, roll);
            assert!(result.hit);
            assert_eq!(result.damage, if roll == 20 { 12 } else { 6 });
        }
    }

    #[test]
    fn test_combat_critical() {
        // Exhaustively check the d20 outcomes; random trials can miss a critical.
        for roll in 1..=20 {
            let result = resolve_attack(0, 20, 5, roll);
            assert_eq!(result.critical, roll == 20);
            assert_eq!(result.hit, roll == 20);
            assert_eq!(result.damage, if roll == 20 { 10 } else { 0 });
        }
    }
}
