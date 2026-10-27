use super::*;

const HELP: &str = "指令：/tp x y z（脚底坐标，支持 ~、~偏移）；/fly [on|off]；/help";

#[derive(Debug, PartialEq)]
enum Command {
    Help,
    Teleport([f32; 3]),
    Fly(Option<bool>),
}

fn parse(text: &str, origin: [f32; 3]) -> Result<Command> {
    let words: Vec<_> = text.split_whitespace().collect();
    match words.as_slice() {
        ["/help"] => Ok(Command::Help),
        ["/fly"] => Ok(Command::Fly(None)),
        ["/fly", "on"] => Ok(Command::Fly(Some(true))),
        ["/fly", "off"] => Ok(Command::Fly(Some(false))),
        ["/tp", x, y, z] => {
            let mut position = [0.0; 3];
            for (i, word) in [x, y, z].into_iter().enumerate() {
                position[i] = if let Some(offset) = word.strip_prefix('~') {
                    origin[i]
                        + if offset.is_empty() {
                            0.0
                        } else {
                            offset.parse::<f32>().context("相对坐标必须是数字。")?
                        }
                } else {
                    word.parse::<f32>().context("坐标必须是数字。")?
                };
            }
            anyhow::ensure!(valid_position(position), "坐标无效或超出世界边界。");
            Ok(Command::Teleport(position))
        }
        ["/tp", ..] => anyhow::bail!("用法：/tp x y z，例如 /tp 28.5 12 32.5 或 /tp ~ ~5 ~"),
        ["/fly", ..] => anyhow::bail!("用法：/fly [on|off]"),
        _ => anyhow::bail!("未知指令。输入 /help 查看帮助。"),
    }
}

impl VoxelWorld {
    pub(super) fn player_command(&mut self, id: u32, text: &str) {
        let result = self.execute_command(id, text);
        if let Some(p) = self.players.get_mut(&id) {
            p.command_log.push(LogEntry {
                text: result.unwrap_or_else(|e| e.to_string()),
                color: 0xBEC9B2,
                turn: self.tick,
            });
            if p.command_log.len() > 4 {
                p.command_log.remove(0);
            }
        }
    }

    pub(super) fn player_log(&self, p: &Player) -> Vec<LogEntry> {
        let mut log: Vec<_> = self.log.iter().chain(&p.command_log).cloned().collect();
        log.sort_by_key(|entry| entry.turn);
        log.drain(..log.len().saturating_sub(12));
        log
    }

    fn execute_command(&mut self, id: u32, text: &str) -> Result<String> {
        let p = self.players.get(&id).context("玩家尚未连接。")?;
        let command = parse(text, p.body.position)?;
        if command == Command::Help {
            return Ok(HELP.into());
        }
        anyhow::ensure!(p.hp > 0, "请等待复活后再使用指令。");
        anyhow::ensure!(self.tick >= p.command_after, "指令操作过快，请稍后重试。");
        // Rate-limit even rejected destinations before potentially generating terrain.
        self.players.get_mut(&id).unwrap().command_after = self.tick + 15;
        match command {
            Command::Help => unreachable!(),
            Command::Fly(value) => {
                let p = self.players.get_mut(&id).unwrap();
                p.flying = value.unwrap_or(!p.flying);
                p.body.velocity = [0.0; 3];
                p.body.grounded = false;
                Ok(if p.flying {
                    "飞行已开启：Space 上升，Shift 下降，Ctrl 加速；松键悬停。"
                } else {
                    "飞行已关闭。"
                }
                .into())
            }
            Command::Teleport(position) => {
                use game_simulation::{HEIGHT, RADIUS};
                anyhow::ensure!(
                    position[0] >= -WORLD_LIMIT as f32 + RADIUS
                        && position[0] < WORLD_LIMIT as f32 - RADIUS
                        && position[2] >= -WORLD_LIMIT as f32 + RADIUS
                        && position[2] < WORLD_LIMIT as f32 - RADIUS
                        && position[1] >= 0.0
                        && position[1] + HEIGHT < WORLD_HEIGHT as f32,
                    "角色身体超出世界边界。"
                );
                let center = column_at(position);
                for z in -1..=1 {
                    for x in -1..=1 {
                        let column = [center[0] + x, center[1] + z];
                        if valid_column(column) {
                            terrain::ensure_column(&mut self.terrain, self.world_seed, column);
                        }
                    }
                }
                anyhow::ensure!(
                    !overlaps(position, &|cell| self.terrain.solid(cell)),
                    "传送落点被方块占用。"
                );
                let collides = |other: [f32; 3]| {
                    (other[0] - position[0]).abs() < RADIUS * 2.0
                        && (other[2] - position[2]).abs() < RADIUS * 2.0
                        && (other[1] - position[1]).abs() < HEIGHT
                };
                anyhow::ensure!(
                    !self
                        .players
                        .iter()
                        .any(|(&other_id, p)| other_id != id && collides(p.body.position))
                        && !self
                            .npcs
                            .query::<&Npc>()
                            .iter()
                            .any(|n| collides(n.body.position)),
                    "传送落点被角色占用。"
                );
                let p = self.players.get_mut(&id).unwrap();
                p.body = Body {
                    position,
                    ..Default::default()
                };
                p.open_container = None;
                p.container_error.clear();
                // Keep queued input sequence numbers: seq only acknowledges executed input.
                Ok(format!(
                    "已传送至 {:.2} {:.2} {:.2}。",
                    position[0], position[1], position[2]
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world() -> (VoxelWorld, mpsc::Receiver<ServerMessage>) {
        let mut w = VoxelWorld::new(42);
        w.npcs.clear();
        let (tx, rx) = mpsc::channel(256);
        w.register_player(1, tx).unwrap();
        (w, rx)
    }

    #[test]
    fn teleport_generates_destination_preserves_sequences_and_closes_container() {
        let (mut w, _rx) = world();
        w.input(
            1,
            1,
            PlayerInput {
                strafe: 1.0,
                ..Default::default()
            },
        )
        .unwrap();
        w.players.get_mut(&1).unwrap().open_container = Some([26, 7, 30]);
        w.execute_command(1, "/tp -200.5 20 300.5").unwrap();
        assert!(w.terrain.nearby_known([-200.5, 20.0, 300.5]));
        let p = &w.players[&1];
        assert_eq!(p.body.position, [-200.5, 20.0, 300.5]);
        assert_eq!(p.body.velocity, [0.0; 3]);
        assert_eq!(p.last_seq, 0);
        assert_eq!(p.inbox.len(), 1);
        assert!(p.open_container.is_none());
        w.tick();
        assert_eq!(w.players[&1].last_seq, 1);
        assert!(w.players[&1].body.position[0] > -200.5);
    }

    #[test]
    fn invalid_destinations_and_rate_limits_leave_player_unchanged() {
        let (mut w, _rx) = world();
        let original = w.players[&1].body;
        let (tx, _other_rx) = mpsc::channel(64);
        w.register_player(2, tx).unwrap();
        let other = w.players[&2].body.position;
        for text in [
            "/tp 0 0 0".into(),
            "/tp 1 31 1".into(),
            "/tp NaN 5 5".into(),
            format!("/tp {} {} {}", other[0], other[1], other[2]),
        ] {
            w.tick += 15;
            assert!(w.execute_command(1, &text).is_err());
            assert_eq!(w.players[&1].body, original);
        }
        w.tick += 15;
        assert!(w.execute_command(1, "/fly on").is_ok());
        assert!(w.execute_command(1, "/fly off").is_err());
        assert!(w.players[&1].flying);
        assert!(w.execute_command(1, "/help").is_ok());
    }

    #[test]
    fn command_feedback_is_private_and_flight_resets_after_respawn_and_reconnect() {
        let (mut w, _rx) = world();
        let (tx, _other_rx) = mpsc::channel(64);
        w.register_player(2, tx).unwrap();
        w.chat(1, "/unknown".into()).unwrap();
        assert!(w
            .player_log(&w.players[&1])
            .iter()
            .any(|e| e.text.contains("未知指令")));
        assert!(w.player_log(&w.players[&2]).is_empty());
        w.chat(1, "hello".into()).unwrap();
        assert!(w
            .player_log(&w.players[&2])
            .iter()
            .any(|e| e.text.contains("hello")));
        w.chat(1, "/fly".into()).unwrap();
        assert!(w.players[&1].flying);
        w.players.get_mut(&1).unwrap().hp = 0;
        w.tick();
        assert!(!w.players[&1].flying);
        w.tick += 15;
        w.chat(1, "/fly".into()).unwrap();
        w.unregister_player(1);
        let (tx, _reconnected) = mpsc::channel(64);
        w.register_player(1, tx).unwrap();
        assert!(!w.players[&1].flying);
    }

    #[test]
    fn parses_absolute_relative_and_rejects_invalid_commands() {
        assert_eq!(
            parse("/tp -20 15 30", [0.0; 3]).unwrap(),
            Command::Teleport([-20.0, 15.0, 30.0])
        );
        assert_eq!(
            parse(" /tp ~ ~2.5 ~-4 ", [2.0, 7.0, 3.0]).unwrap(),
            Command::Teleport([2.0, 9.5, -1.0])
        );
        assert_eq!(parse("/fly", [0.0; 3]).unwrap(), Command::Fly(None));
        assert_eq!(
            parse("/fly off", [0.0; 3]).unwrap(),
            Command::Fly(Some(false))
        );
        for invalid in [
            "/tp NaN 10 0",
            "/tp inf 10 0",
            "/tp 32768 10 0",
            "/tp 0 32 0",
            "/tp 1 2",
            "/tp 1 2 3 4",
            "/fly maybe",
            "/help extra",
            "/unknown",
        ] {
            assert!(parse(invalid, [0.0; 3]).is_err(), "{invalid}");
        }
    }
}
