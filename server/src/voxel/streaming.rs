use super::*;

impl VoxelWorld {
    /// Data only, on the authority thread. Never generate a column twice.
    pub(super) fn stream_chunks(&mut self) {
        for p in self.players.values_mut() {
            let center = column_at(p.body.position);
            let mut wanted = Vec::new();
            for z in -VIEW_RADIUS..=VIEW_RADIUS {
                for x in -VIEW_RADIUS..=VIEW_RADIUS {
                    let c = [center[0] + x, center[1] + z];
                    if valid_column(c) {
                        wanted.push(c);
                    }
                }
            }
            wanted.sort_by_key(|c| {
                let x = c[0] - center[0];
                let z = c[1] - center[1];
                (x.abs().max(z.abs()), x * x + z * z, *c)
            });
            let obsolete: Vec<_> = p
                .sent_columns
                .iter()
                .copied()
                .filter(|c| !wanted.contains(c))
                .collect();
            // Reserve room for live snapshots; a slow peer cannot block the tick.
            let mut budget = 8;
            for c in obsolete {
                if budget == 0 || p.tx.capacity() <= 2 {
                    break;
                }
                if p.tx
                    .try_send(ServerMessage::UnloadColumn { column: c })
                    .is_err()
                {
                    break;
                }
                p.sent_columns.remove(&c);
                budget -= 1;
            }
            // Finish unloading before adding: cache size stays bounded even after teleport.
            if p.sent_columns.iter().any(|c| !wanted.contains(c)) {
                continue;
            }
            for c in wanted {
                if p.sent_columns.contains(&c) {
                    continue;
                }
                if budget < 2 || p.tx.capacity() < 4 {
                    break;
                }
                terrain::ensure_column(&mut self.terrain, self.world_seed, c);
                let mut sent = true;
                for y in (0..WORLD_HEIGHT).step_by(16) {
                    let origin = [c[0] * 16, y, c[1] * 16];
                    if p.tx
                        .try_send(ServerMessage::Chunk3d {
                            origin,
                            blocks: self.terrain.chunk(origin),
                        })
                        .is_err()
                    {
                        sent = false;
                        break;
                    }
                }
                if !sent {
                    break;
                }
                p.sent_columns.insert(c);
                budget -= 2;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn drain(rx: &mut mpsc::Receiver<ServerMessage>, cache: &mut Grid) -> usize {
        let mut count = 0;
        while let Ok(m) = rx.try_recv() {
            count += 1;
            match m {
                ServerMessage::Chunk3d { origin, blocks } => {
                    assert!(cache.apply_chunk(origin, &blocks));
                }
                ServerMessage::UnloadColumn { column } => cache.unload_column(column),
                _ => panic!("unexpected stream message"),
            }
            assert!(cache.columns().len() <= 49);
        }
        count
    }
    #[test]
    fn bounded_stream_unloads_revisits_and_never_overwrites_edits() {
        let mut w = VoxelWorld::new(12345);
        let (tx, mut rx) = mpsc::channel(64);
        w.register_player(1, tx).unwrap();
        assert!(
            rx.try_recv().is_err(),
            "login does not queue the entire map"
        );
        let mut cache = Grid::unknown();
        for _ in 0..13 {
            w.stream_chunks();
            assert!(drain(&mut rx, &mut cache) <= 8);
        }
        assert_eq!(cache.columns().len(), 49);
        assert!(cache.nearby_known(w.players[&1].body.position));
        w.stream_chunks();
        assert_eq!(drain(&mut rx, &mut cache), 0);
        let edited = [28, 6, 32];
        w.terrain.set(edited, PLANK);
        let original = w.players[&1].body;
        w.players.get_mut(&1).unwrap().body.position = [-128.5, 10.0, 160.5];
        for _ in 0..21 {
            w.stream_chunks();
            assert!(drain(&mut rx, &mut cache) <= 8);
        }
        assert_eq!(cache.columns().len(), 49);
        assert!(cache.nearby_known(w.players[&1].body.position));
        assert_eq!(cache.get(edited), 255);
        assert_eq!(w.terrain.get(edited), PLANK);
        w.players.get_mut(&1).unwrap().body = original;
        for _ in 0..21 {
            w.stream_chunks();
            drain(&mut rx, &mut cache);
        }
        assert_eq!(cache.get(edited), PLANK);
        assert!(cache
            .columns()
            .iter()
            .all(|c| (c[0] - 1).abs() <= 3 && (c[1] - 2).abs() <= 3));
    }
    #[test]
    fn full_queue_does_not_mark_unsent_chunks_or_generate_them() {
        let mut w = VoxelWorld::new(1);
        let (tx, mut rx) = mpsc::channel(4);
        tx.try_send(ServerMessage::Pong { seq: 1 }).unwrap();
        w.register_player(1, tx).unwrap();
        w.players.get_mut(&1).unwrap().body.position = [-100.5, 10.0, -100.5];
        let before = w.terrain.blocks.len();
        w.stream_chunks();
        assert!(w.players[&1].sent_columns.is_empty());
        assert_eq!(w.terrain.blocks.len(), before);
        rx.try_recv().unwrap();
        w.stream_chunks();
        assert_eq!(w.players[&1].sent_columns.len(), 1);
        assert_eq!(w.terrain.blocks.len(), before + 2);
    }
    #[test]
    fn deterministic_generation_crosses_negative_seams_and_respects_limit() {
        let mut a = Grid::unknown();
        let mut b = Grid::unknown();
        let columns = [[-2, -1], [-1, -1], [-1, 0], [4, 0]];
        for c in columns {
            terrain::ensure_column(&mut a, 314, c);
        }
        for c in columns.into_iter().rev() {
            terrain::ensure_column(&mut b, 314, c);
        }
        assert_eq!(a, b);
        assert_ne!(a.get([-1, 0, -1]), 255);
        assert_ne!(a.get([-16, 0, -1]), 255);
        let before = a.clone();
        terrain::ensure_column(&mut a, 9, [WORLD_LIMIT / 16, 0]);
        assert_eq!(a, before);
    }
}
