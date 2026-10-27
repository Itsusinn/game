use super::*;

#[test]
fn natural_spawn_is_safe_seeded_and_supports_sixteen_players() {
    let mut heights = BTreeSet::new();
    for seed in [0, 1, 2, 7, 17, 42, 12345, 99999] {
        let mut world = VoxelWorld::new(seed).unwrap();
        let mut receivers = Vec::new();
        for id in 1..=16 {
            let (tx, rx) = mpsc::channel(64);
            world.register_player(id, tx).unwrap();
            receivers.push(rx);
            let p = world.players[&id].body.position;
            assert!(!overlaps(p, &|v| world.terrain.solid(v)));
            assert_eq!(
                terrain::standing_position(&world.terrain, p[0] as i32, p[2] as i32),
                Some(p)
            );
            heights.insert(p[1] as i32);
        }
        assert_eq!(
            world.containers.len(),
            1,
            "seed {seed} must retain an initial chest"
        );
        assert_eq!(world.npcs.len(), 3);
        for npc in world.npcs.query::<&Npc>().iter() {
            assert_eq!(
                terrain::standing_position(
                    &world.terrain,
                    npc.body.position[0] as i32,
                    npc.body.position[2] as i32
                ),
                Some(npc.body.position)
            );
        }
        println!(
            "SPAWN seed={seed} player={:?} chest={:?}",
            world.players[&1].body.position,
            world.containers.keys().collect::<Vec<_>>()
        );
        if seed == 12345 {
            let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("../target");
            std::fs::create_dir_all(&folder).unwrap();
            let p = world.players[&1].body.position;
            let chest = world.containers.keys().next().unwrap();
            std::fs::write(
                folder.join("spawn-site.json"),
                serde_json::to_vec_pretty(&serde_json::json!({"spawn":p,"chest":chest})).unwrap(),
            )
            .unwrap();
        }
    }
    assert!(heights.len() > 3);
    let a = VoxelWorld::new(12345).unwrap();
    let b = VoxelWorld::new(12345).unwrap();
    assert_eq!(a.terrain, b.terrain);
    assert_eq!(a.spawn_sites, b.spawn_sites);
}

#[test]
fn natural_spawn_matches_streamed_terrain_and_survives_reopen_and_edits() {
    let mut w = VoxelWorld::new(12345).unwrap();
    let mut generated = Grid::unknown();
    for c in w.terrain.columns() {
        terrain::ensure_column(&mut generated, 12345, c).unwrap();
    }
    for p in w.containers.keys() {
        generated.set(*p, CHEST);
    }
    assert_eq!(
        w.terrain, generated,
        "initial columns use exactly the streaming generator"
    );
    let p = w.spawn_position(0).unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../target/spawn-tests");
    std::fs::create_dir_all(&root).unwrap();
    let dir = tempfile::tempdir_in(root).unwrap();
    let path = dir.path().join("world3d.bin");
    w.save(&path).unwrap();
    let restored = VoxelWorld::load(&path).unwrap();
    assert_eq!(restored.terrain, w.terrain);
    assert_eq!(restored.spawn_position(0), Some(p));
    let (tx, _rx) = mpsc::channel(256);
    w.register_player(1, tx).unwrap();
    w.terrain.set(p.map(|v| v.floor() as i32), STONE);
    w.chat(1, "/spawn".into()).unwrap();
    assert_ne!(w.players[&1].body.position, p);
    w.players.get_mut(&1).unwrap().hp = 0;
    w.tick();
    assert_eq!(w.players[&1].hp, 100);
    assert!(!overlaps(w.players[&1].body.position, &|p| w
        .terrain
        .solid(p)));
    let before = w.terrain.clone();
    for c in w.terrain.columns() {
        terrain::ensure_column(&mut w.terrain, 999, c).unwrap();
    }
    assert_eq!(w.terrain, before);
}

#[test]
fn spawn_rejects_water_trees_unknown_and_unsupported_air() {
    let mut g = Grid::empty();
    assert_eq!(terrain::standing_position(&g, 20, 20), None);
    for block in [WATER, ICE, WOOD, LEAVES, PINE_LEAVES, BIRCH_LEAVES, CHEST] {
        g.set([20, 10, 20], block);
        assert_eq!(terrain::standing_position(&g, 20, 20), None);
    }
    g.set([20, 10, 20], GRASS);
    assert_eq!(
        terrain::standing_position(&g, 20, 20),
        Some([20.5, 11.0, 20.5])
    );
    g.unload_column([1, 1]);
    assert_eq!(terrain::standing_position(&g, 20, 20), None);
}
