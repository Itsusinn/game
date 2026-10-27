use super::*;
use crate::player_data::{self, PlayerArchive, PlayerData, DATA_VERSION};

impl Player {
    pub(super) fn saved_data(&self, uuid: [u8; 16]) -> Result<PlayerData> {
        PlayerData::capture(
            uuid,
            self.name.clone(),
            self.body,
            self.hp,
            [self.yaw, self.pitch],
            &self.inventory,
        )
    }
}

impl VoxelWorld {
    pub fn register_profile(
        &mut self,
        id: u32,
        uuid: [u8; 16],
        name: String,
        tx: mpsc::Sender<ServerMessage>,
    ) -> Result<[f32; 2]> {
        anyhow::ensure!(uuid != [0; 16], "无效玩家 UUID。");
        anyhow::ensure!(
            !name.trim().is_empty() && name.len() <= 128,
            "无效玩家昵称。"
        );
        anyhow::ensure!(
            !self.players.values().any(|p| p.uuid == Some(uuid)),
            "该玩家 UUID 已在线，请关闭另一个客户端或使用不同身份文件。"
        );
        anyhow::ensure!(
            self.saved_players.contains_key(&uuid)
                || self.saved_players.len() + self.players.len() < 10_000,
            "player storage capacity reached"
        );
        let restored = self
            .saved_players
            .get(&uuid)
            .map(|record| {
                Ok::<_, anyhow::Error>((record.restore()?, record.health, record.rotation))
            })
            .transpose()?;
        self.register_player(id, tx)?;
        let safe = restored.as_ref().is_some_and(|((body, _), health, _)| {
            *health > 0.0 && self.position_available(body.position, id)
        });
        let p = self.players.get_mut(&id).unwrap();
        p.uuid = Some(uuid);
        p.name = name;
        if let Some(((body, inventory), health, rotation)) = restored {
            p.inventory = inventory;
            p.yaw = rotation[0];
            p.pitch = rotation[1];
            p.hp = if health > 0.0 { health as i32 } else { 100 };
            if safe {
                p.body = body;
            }
        }
        Ok([p.yaw, p.pitch])
    }

    fn position_available(&self, position: [f32; 3], exclude: u32) -> bool {
        position
            .iter()
            .zip(WORLD_SIZE)
            .all(|(v, n)| v.is_finite() && *v >= 0.0 && *v < n as f32)
            && !overlaps(position, &|p| self.terrain.solid(p))
            && self
                .players
                .iter()
                .all(|(&id, p)| id == exclude || distance(p.body.position, position) > 1.0)
            && self
                .npcs
                .query::<&Npc>()
                .iter()
                .all(|n| distance(n.body.position, position) > 1.0)
    }

    pub fn set_save_path(&mut self, path: std::path::PathBuf) {
        self.save_path = Some(path);
    }

    pub fn checkpoint(&self) -> Result<()> {
        if let Some(path) = &self.save_path {
            self.save(path)?;
        }
        Ok(())
    }

    pub(super) fn write_player_checkpoint(&self, path: &Path) -> Result<String> {
        let mut records = self.saved_players.clone();
        for p in self.players.values() {
            if let Some(uuid) = p.uuid {
                records.insert(uuid, p.saved_data(uuid)?);
            }
        }
        let folder = path.with_extension("players");
        std::fs::create_dir_all(&folder)?;
        let generation = uuid::Uuid::new_v4().to_string();
        player_data::write_new(
            &folder.join(format!("{generation}.rkyv")),
            &PlayerArchive {
                data_version: DATA_VERSION,
                players: records.into_values().collect(),
            },
        )?;
        // World rename is the commit point. Interrupted writes leave an unused
        // immutable generation; the previous world still names its exact data.
        Ok(generation)
    }

    pub(super) fn load_player_checkpoint(
        &mut self,
        path: &Path,
        generation: String,
        items: Vec<GroundItem3d>,
    ) -> Result<()> {
        let id =
            uuid::Uuid::parse_str(&generation).context("invalid player checkpoint reference")?;
        anyhow::ensure!(
            id.to_string() == generation,
            "noncanonical player checkpoint reference"
        );
        let archive = player_data::read(
            &path
                .with_extension("players")
                .join(format!("{generation}.rkyv")),
        )?;
        let mut ids = std::collections::HashSet::new();
        anyhow::ensure!(items.len() <= 100_000, "too many saved ground items");
        for item in &items {
            anyhow::ensure!(
                ids.insert(item.id)
                    && !item.name.is_empty()
                    && item.name.len() <= 128
                    && item
                        .position
                        .iter()
                        .zip(WORLD_SIZE)
                        .all(|(v, n)| v.is_finite() && *v >= 0.0 && *v < n as f32),
                "invalid saved ground item"
            );
        }
        self.saved_players = archive.players.into_iter().map(|p| (p.uuid, p)).collect();
        self.items = items;
        Ok(())
    }
}

/// The same exclusive world lock is used by standalone and embedded servers.
pub fn lock_world(path: &Path) -> Result<std::fs::File> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path.with_extension("bin.lock"))?;
    lock.try_lock().context("this world is already open")?;
    Ok(lock)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn directory() -> tempfile::TempDir {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../target/player-world-tests");
        std::fs::create_dir_all(&root).unwrap();
        tempfile::tempdir_in(root).unwrap()
    }
    fn register(w: &mut VoxelWorld, id: u32, uuid: [u8; 16], name: &str) {
        let (tx, _rx) = mpsc::channel(64);
        w.register_profile(id, uuid, name.into(), tx).unwrap();
    }

    #[test]
    fn checkpoint_restores_uuid_slots_health_motion_and_camera_after_rename() {
        let dir = directory();
        let path = dir.path().join("world3d.bin");
        let mut w = VoxelWorld::new(12345);
        w.npcs.clear();
        register(&mut w, 1, [1; 16], "Before");
        let p = w.players.get_mut(&1).unwrap();
        p.body.position = [28.5, 8.0, 32.5];
        p.body.velocity = [0.0, -2.0, 0.0];
        p.body.grounded = false;
        p.hp = 73;
        p.yaw = 1.25;
        p.pitch = -0.4;
        p.inventory.slots[35] = Some(InventoryEntry {
            name: "Field supplies".into(),
            quantity: 64,
        });
        p.inventory.selected = 8;
        let expected = p.saved_data([1; 16]).unwrap();
        w.items.clear();
        w.unregister_player(1);
        register(&mut w, 2, [2; 16], "Other");
        w.save(&path).unwrap();
        let mut restored = VoxelWorld::load(&path).unwrap();
        assert!(
            restored.items.is_empty(),
            "picked-up world items must not respawn"
        );
        register(&mut restored, 77, [1; 16], "After");
        let mut actual = restored.players[&77].saved_data([1; 16]).unwrap();
        assert_eq!(actual.name, "After");
        actual.name = "Before".into();
        assert_eq!(actual, expected);
        let (tx, _rx) = mpsc::channel(64);
        assert!(restored
            .register_profile(78, [1; 16], "Clone".into(), tx)
            .is_err());
        assert_eq!(restored.player_count(), 1);
        restored.save(&path).unwrap();
        assert_eq!(
            VoxelWorld::load(&path).unwrap().saved_players.len(),
            2,
            "offline players survive subsequent saves"
        );
    }

    #[test]
    fn interrupted_commit_never_mixes_chest_and_inventory_generations() {
        let dir = directory();
        let path = dir.path().join("world3d.bin");
        let mut w = VoxelWorld::new(1);
        w.npcs.clear();
        register(&mut w, 1, [1; 16], "Player");
        w.players.get_mut(&1).unwrap().inventory.slots[0] = Some(InventoryEntry {
            name: "Field supplies".into(),
            quantity: 5,
        });
        w.save(&path).unwrap();
        let original = std::fs::read(&path).unwrap();
        let position = [26, 7, 30];
        w.players.get_mut(&1).unwrap().body.position = [26.5, 7.0, 32.0];
        w.container_command(1, ContainerCommand::Open { position });
        w.container_command(
            1,
            ContainerCommand::Transfer {
                position,
                revision: 0,
                inventory_revision: 0,
                slot: 0,
                target: None,
                quantity: 2,
                deposit: true,
            },
        );
        assert_eq!(w.containers[&position].items[0].quantity, 2);
        let old_block = w.terrain.get([1, 20, 3]);
        w.terrain.set([1, 20, 3], STONE);
        w.items.clear();
        std::fs::create_dir(path.with_extension("tmp")).unwrap();
        assert!(w.save(&path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), original);
        let before = VoxelWorld::load(&path).unwrap();
        assert_eq!(before.saved_players[&[1; 16]].inventory[0].count, 5);
        assert!(before.containers[&position].items.is_empty());
        assert_eq!(before.terrain.get([1, 20, 3]), old_block);
        assert!(!before.items.is_empty());
        std::fs::remove_dir(path.with_extension("tmp")).unwrap();
        w.save(&path).unwrap();
        let after = VoxelWorld::load(&path).unwrap();
        assert_eq!(after.saved_players[&[1; 16]].inventory[0].count, 3);
        assert_eq!(after.containers[&position].items[0].quantity, 2);
        assert_eq!(after.terrain.get([1, 20, 3]), STONE);
        assert!(after.items.is_empty());
        let bytes = std::fs::read(&path).unwrap();
        let save: Save = rmp_serde::from_slice(&bytes[8..]).unwrap();
        let archive = path
            .with_extension("players")
            .join(format!("{}.rkyv", save.player_archive.unwrap()));
        std::fs::write(&archive, b"broken").unwrap();
        assert!(VoxelWorld::load(&path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        assert_eq!(std::fs::read(archive).unwrap(), b"broken");
    }

    #[test]
    fn v2_backup_and_blocked_position_preserve_inventory() {
        let dir = directory();
        let path = dir.path().join("world3d.bin");
        let w = VoxelWorld::new(1);
        let mut bytes = b"DNG3D001".to_vec();
        bytes.extend(
            rmp_serde::to_vec(&(
                2u32,
                WORLD_SIZE,
                1u64,
                &w.terrain.blocks,
                w.containers.values().collect::<Vec<_>>(),
            ))
            .unwrap(),
        );
        std::fs::write(&path, &bytes).unwrap();
        let mut world = VoxelWorld::load(&path).unwrap();
        assert_eq!(std::fs::read(path.with_extension("v2.bak")).unwrap(), bytes);
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        register(&mut world, 1, [1; 16], "Player");
        world.players.get_mut(&1).unwrap().inventory.selected = 6;
        world.unregister_player(1);
        world.saved_players.get_mut(&[1; 16]).unwrap().pos = [0.0, 0.0, 0.0];
        register(&mut world, 2, [1; 16], "Renamed");
        assert_ne!(world.players[&2].body.position, [0.0, 0.0, 0.0]);
        assert_eq!(world.players[&2].inventory.selected, 6);
        world.save(&path).unwrap();
        assert!(VoxelWorld::load(&path).is_ok());
        assert_eq!(std::fs::read(path.with_extension("v2.bak")).unwrap(), bytes);
        let blocked = dir.path().join("blocked.bin");
        std::fs::write(&blocked, &bytes).unwrap();
        std::fs::create_dir(blocked.with_extension("v2.bak")).unwrap();
        assert!(VoxelWorld::load(&blocked).is_err());
        assert_eq!(std::fs::read(blocked).unwrap(), bytes);
    }

    #[test]
    fn v3_migration_keeps_players_chests_items_and_original_archive() {
        let dir = directory();
        let path = dir.path().join("world3d.bin");
        let mut w = VoxelWorld::new(47);
        register(&mut w, 1, [3; 16], "Legacy");
        w.players.get_mut(&1).unwrap().inventory.selected = 8;
        w.containers
            .get_mut(&[26, 7, 30])
            .unwrap()
            .items
            .push(InventoryEntry {
                name: "Supplies".into(),
                quantity: 99,
            });
        let generation = w.write_player_checkpoint(&path).unwrap();
        let player_path = path
            .with_extension("players")
            .join(format!("{generation}.rkyv"));
        let player_bytes = std::fs::read(&player_path).unwrap();
        let mut original = b"DNG3D001".to_vec();
        // Seven-field v3 fixture predates the chunk reference field.
        original.extend(
            rmp_serde::to_vec(&(
                3u32,
                WORLD_SIZE,
                w.world_seed,
                &w.terrain.blocks,
                w.containers.values().collect::<Vec<_>>(),
                Some(generation),
                Some(&w.items),
            ))
            .unwrap(),
        );
        std::fs::write(&path, &original).unwrap();
        let loaded = VoxelWorld::load(&path).unwrap();
        assert_eq!(loaded.terrain.blocks, w.terrain.blocks);
        assert_eq!(loaded.containers, w.containers);
        assert_eq!(loaded.items, w.items);
        assert_eq!(loaded.saved_players[&[3; 16]].selected_item_slot, 8);
        assert_eq!(std::fs::read(&path).unwrap(), original);
        assert_eq!(
            std::fs::read(path.with_extension("v3.bak")).unwrap(),
            original
        );
        loaded.save(&path).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        let save: Save = rmp_serde::from_slice(&bytes[8..]).unwrap();
        assert_eq!(save.version, 4);
        assert!(save.blocks.is_empty() && save.containers.is_empty() && save.items.is_none());
        assert_eq!(save.player_archive, save.chunk_archive);
        let restored = VoxelWorld::load(&path).unwrap();
        assert_eq!(restored.terrain.blocks, loaded.terrain.blocks);
        assert_eq!(restored.containers, loaded.containers);
        assert_eq!(restored.items, loaded.items);
        assert_eq!(restored.saved_players, loaded.saved_players);
        assert_eq!(std::fs::read(player_path).unwrap(), player_bytes);
        let chunk_path = path
            .with_extension("chunks")
            .join(format!("{}.rkyv", save.chunk_archive.unwrap()));
        let chunk_bytes = std::fs::read(&chunk_path).unwrap();
        std::fs::remove_file(&chunk_path).unwrap();
        assert!(VoxelWorld::load(&path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        std::fs::write(&chunk_path, &chunk_bytes[..chunk_bytes.len() - 1]).unwrap();
        assert!(VoxelWorld::load(&path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }
}
