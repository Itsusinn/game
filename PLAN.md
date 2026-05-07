# CDDA 风格 2D 地牢回合制游戏 — 完整实施计划

## 架构概览

```
┌─────────────────────────────────────────────────────┐
│                 Rust Server (tokio)                  │
│                                                      │
│  tokio::spawn (全异步)                               │
│  ┌──────────────────────────────────────────────┐   │
│  │  WorldManager                                 │   │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐      │   │
│  │  │ SubWorld │ │ SubWorld │ │ SubWorld │ ...  │   │
│  │  │ (0,0)    │ │ (0,1)    │ │ (1,0)    │      │   │
│  │  │ select!  │ │ select!  │ │ select!  │      │   │
│  │  │ loop     │ │ loop     │ │ loop     │      │   │
│  │  └────┬─────┘ └────┬─────┘ └────┬─────┘      │   │
│  │       │             │            │             │   │
│  │  cross_world_tx ───┼────────────┘             │   │
│  │       │             │                          │   │
│  └───────┼─────────────┼──────────────────────────┘   │
│          │             │                               │
│  ┌───────▼─────────────▼──────────────────────────┐   │
│  │  Network Layer (QUIC quinn)                     │   │
│  │  TCP-like over UDP :9876                        │   │
│  └───────┬─────────────┬──────────────────────────┘   │
└──────────┼─────────────┼──────────────────────────────┘
           │             │        QUIC (MessagePack)
           │             │
┌──────────▼─────────────▼──────────────────────────────┐
│              Godot Client (×N 个玩家)                   │
│                                                         │
│  ┌──────────────────────────────────────────────────┐  │
│  │  Rust — GDExtension                               │  │
│  │                                                    │  │
│  │  ┌──────────────┐  ┌───────────────────────────┐  │  │
│  │  │ network      │  │ state                     │  │  │
│  │  │  QUIC dial   │─▶│ 反序列化+GameState缓存    │  │  │
│  │  │  send/recv   │  │ apply_snapshot()          │  │  │
│  │  └──────────────┘  │ get_visible_tiles()       │  │  │
│  │                     │ get_entities()            │  │  │
│  │  ┌──────────────┐  │ get_player_status()       │  │  │
│  │  │ prediction   │  └───────────┬───────────────┘  │  │
│  │  │ 本地移动预测  │              │                  │  │
│  │  │ 输入序列号    │              │ get_X() API      │  │
│  │  │ rollback     │              ▼                  │  │
│  │  └──────────────┘  ┌───────────────────────────┐  │  │
│  │  ┌──────────────┐  │ godot_bridge              │  │  │
│  │  │ input_queue  │──▶ 暴露给 GDScript 的高等级  │  │  │
│  │  │ pending[]    │  │ 方法 (非 bytes)            │  │  │
│  │  │ ack(seq)     │  └───────────┬───────────────┘  │  │
│  │  └──────────────┘              │                  │  │
│  └────────────────────────────────┼──────────────────┘  │
│                                   │                      │
│  ┌────────────────────────────────▼──────────────────┐  │
│  │  GDScript (极薄)                                   │  │
│  │  RustClient.tick(delta)   ← 每帧驱动网络+预测     │  │
│  │  TileMap.sync(tiles)      ← 渲染可见地图         │  │
│  │  Entity.sync(entities)    ← 渲染实体             │  │
│  │  Camera.position = pos    ← 跟随                 │  │
│  │  HUD.update(status)       ← HP/体力/背包         │  │
│  └──────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────┘
```

---

## 项目目录结构

```
cdda-rust/
├── Cargo.toml                         # workspace 根
├── protocol/                          # 共享消息定义
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── messages.rs                # ClientMessage / ServerMessage
│       └── types.rs                   # Coord, TileData, EntityData, 常量
│
├── server/                            # Rust 服务器
│   ├── Cargo.toml                     # tokio, quinn, rmp-serde, protocol
│   └── src/
│       ├── main.rs                    # tokio::main 入口
│       ├── network/
│       │   └── mod.rs                 # QUIC listener + per-connection handler
│       ├── world/
│       │   ├── mod.rs
│       │   ├── world_manager.rs       # 子世界路由 + 创建/销毁
│       │   ├── sub_world.rs           # select! 异步游戏循环
│       │   ├── chunk_manager.rs       # 区块加载/卸载
│       │   ├── map.rs                 # GameMap 实现
│       │   ├── tile.rs                # Tile 定义 + flags
│       │   ├── entity.rs              # Entity + EntityManager
│       │   ├── turn_system.rs         # 行动点数调度
│       │   ├── fov.rs                 # 递归阴影投射
│       │   ├── pathfinding.rs         # A* 寻路
│       │   ├── combat.rs              # 近战/远程/护甲
│       │   ├── ai.rs                  # 敌人状态机
│       │   ├── item.rs                # 物品系统
│       │   ├── worldgen.rs            # 确定性随机世界生成
│       │   └── messagelog.rs          # 消息日志
│       └── storage/
│           └── save.rs
│
├── client/                            # GDExtension Rust (编译为 .so)
│   ├── Cargo.toml                     # godot, quinn, rmp-serde, protocol
│   └── src/
│       ├── lib.rs                     # gdextension 入口 + GameClient Godot 类
│       ├── network_client.rs          # QUIC 连接、发送、接收
│       ├── state.rs                   # GameState 缓存 + 反序列化
│       ├── prediction.rs              # 本地预测 + rollback
│       ├── input_queue.rs             # 序列号 + 待确认队列
│       └── godot_bridge.rs            # tick(), get_*(), send_*() 高等级 API
│
└── godot_project/
    ├── project.godot
    ├── bin/
    │   ├── cdda_client.gdextension    # GDExtension 加载配置
    │   └── libclient.so               # 编译产物 (gitignore)
    ├── scenes/
    │   ├── main.tscn
    │   ├── hud.tscn
    │   └── inventory.tscn
    └── scripts/
        ├── game_manager.gd            # ~30 行, 只调 Rust API
        ├── map_view.gd                # TileMap 渲染
        ├── entity_view.gd             # 实体 Sprite
        ├── input_handler.gd           # 键盘→Rust
        └── hud.gd                     # HP/体力/物品/日志
```

---

## 阶段 0：项目骨架

| 步骤 | 操作 | 文件 |
|------|------|------|
| 0.1 | 创建 workspace | `Cargo.toml` (workspace) |
| 0.2 | protocol crate | `protocol/Cargo.toml`, `src/lib.rs`, `messages.rs`, `types.rs` |
| 0.3 | server main 骨架 | `server/Cargo.toml`, `src/main.rs` (空 tokio::main) |
| 0.4 | client 独立测试二进制 | `client/Cargo.toml`, `src/main.rs` (非 GDExtension, 纯 QUIC 测试) |

### 0.2 protocol — 消息定义

```rust
// 核心枚举
enum ClientMessage {
    Login { version, player_name },
    PlayerAction { seq: u32, action: ActionType, target: Option<Coord>, params },
    Logout,
    Ping { seq: u32 },
}

enum ActionType {
    MoveUp/Down/Left/Right/UpLeft/UpRight/DownLeft/DownRight,
    Wait, MeleeAttack, RangedAttack, Pickup, Drop,
    UseItem, Craft, Inspect, Interact, Chat { text },
}

enum ServerMessage {
    LoginAccepted { player_id, sub_world_id, world_seed },
    WorldState {
        seq, player_pos,
        visible_tiles: Vec<TileData>,
        explored: Vec<bool>, explored_width: u32,
        entities: Vec<EntityData>,
        items_ground: Vec<(Coord, ItemStack)>,
        message_log: Vec<LogEntry>,
        hp, stamina, thirst, hunger,
    },
    EntityMoved { id, from, to },
    AttackResult { attacker, target, damage, killed },
    ChatMessage { player_id, player_name, text },
    SubWorldTransfer { new_sub_world_id, pos },
    EntityJoined { entity: EntityData },
    EntityLeft { id },
    ChunkData { cx, cy, tiles },
    Error { code, text },
    Pong { seq },
}
```

**序列化**：`rmp-serde` (MessagePack)

**帧格式**：`[4字节大端长度 (u32)][MessagePack 字节]`

---

## 阶段 1：网络层调通

| 步骤 | 操作 | 关键代码 |
|------|------|---------|
| 1.1 | 服务器 QUIC listener + 自签名证书 | `server/src/network/mod.rs` — `make_server_config()`, `run_server()` |
| 1.2 | 服务器 accept + per-connection loop | `handle_connection()` → `connection.accept_bi()` |
| 1.3 | 服务器流处理：长度前缀 + deserialize | `handle_stream()` |
| 1.4 | 客户端测试二进制 QUIC dial | `client/src/main.rs` (临时) |
| 1.5 | 客户端发送测试消息 + 接收 echo | 验证完整回路 |

### 证书方案（开发环境）

```
rcgen 生成自签名证书
服务器: rustls::ServerConfig → QuicServerConfig → quinn::ServerConfig
客户端: SkipCertVerification (dangerous, 仅开发用)
```

### 核心 API 对齐 (quinn 0.11)

```
Endpoint::accept() → Incoming
Incoming.accept()  → Result<Connecting>
Connecting.await   → Result<Connection>

Connection.accept_bi() → Result<(SendStream, RecvStream)>

QuicServerConfig::try_from(rustls::ServerConfig) → Result
ServerConfig {
    crypto: Arc<impl crypto::ServerConfig>,
    transport: Arc<TransportConfig>,
}
```

---

## 阶段 2：子世界系统（全异步）

| 步骤 | 操作 |
|------|------|
| 2.1 | Tile 定义 + flags（通行/透明/家具） |
| 2.2 | GameMap + 基础数据结构 |
| 2.3 | Entity + EntityManager |
| 2.4 | WorldManager：子世界路由、创建/销毁 |
| 2.5 | SubWorld：`tokio::select!` 循环 |
| 2.6 | 玩家加入/离开消息处理 |
| 2.7 | WorldGen：确定性 seed → 房间+走廊 |
| 2.8 | 跨子世界 channel 通信 |

### SubWorld 异步循环

```rust
pub async fn run(&mut self, mut rx: mpsc::Receiver<SubWorldCmd>) {
    let mut ai_interval = tokio::time::interval(Duration::from_millis(200));

    loop {
        tokio::select! {
            cmd = rx.recv() => {
                match cmd {
                    Some(c) => self.handle_cmd(c).await,
                    None => break,
                }
            }
            _ = ai_interval.tick() => {
                if !self.players.is_empty() {
                    self.advance_ai();
                    self.broadcast_all().await;
                }
            }
            msg = self.cross_world_rx.recv() => {
                self.handle_cross_world(msg);
            }
        }
    }
}
```

### 子世界分配规则

```
CHUNK_SIZE  = 32 tiles
REGION_SIZE = 16 chunks
SUBWORLD_SIZE = 512 tiles

SubWorldId = (coord_x.div_euclid(512), coord_y.div_euclid(512))
```

### WorldManager API

```rust
impl WorldManager {
    fn allocate_player_id() -> u32
    async fn get_or_create_sub_world(id: (i64,i64)) -> Sender<SubWorldCmd>
    async fn register_player(player_id, sub_world_id)
    async fn unregister_player(player_id)
    async fn transfer_player(player_id, from_sw, to_sw, new_pos)
}
```

---

## 阶段 3：客户端状态管理

| 步骤 | 模块 | 说明 |
|------|------|------|
| 3.1 | `state.rs` | `GameState` 结构体：缓存可见 tiles、实体列表、玩家状态、消息日志 |
| 3.2 | `state.rs` | `apply_snapshot(msg: ServerMessage)` — 全量更新状态 |
| 3.3 | `state.rs` | `apply_delta(msg: ServerMessage)` — 增量更新（EntityMoved 等） |
| 3.4 | `input_queue.rs` | `PendingAction { seq, action, predicted_state }` |
| 3.5 | `input_queue.rs` | `ack(seq)` — 服务器确认后移除，对比预测 |
| 3.6 | `prediction.rs` | `predict_move()` — 本地立即更新坐标 |
| 3.7 | `prediction.rs` | `rollback()` — 服务器位置 vs 预测位置不一时回滚 |

### 预测流程

```
frame 1:
  玩家按 W  →  prediction: 本地 pos.y -= 1
            →  input_queue: push { seq: 42, action: MoveUp, pos: new_pos }
            →  network: send ClientMessage::PlayerAction { seq:42, MoveUp }

frame 2-5 (网络延迟):
  渲染用的是 predicted pos (本地已经移动了)
  input_queue 里 seq:42 待确认

frame 6:
  network: recv ServerMessage::WorldState { seq: 42, player_pos: (x, y-1) }
  对比 prediction → 一致 → input_queue.remove(42)
  如果不一致 → rollback: override predicted pos with server pos
```

---

## 阶段 4：Godot 集成

| 步骤 | 操作 |
|------|------|
| 4.1 | 下载 Godot 4.4+，验证运行 |
| 4.2 | 确定 `godot` crate 正确版本（`cargo search godot`） |
| 4.3 | client Cargo.toml + `cdylib` + `godot` 依赖 |
| 4.4 | `lib.rs` — `#[gdextension]` + `GameClient` Godot 类注册 |
| 4.5 | `godot_bridge.rs` — 暴露 API |

### godot_bridge API

```rust
#[godot_api]
impl GameClient {
    // 生命周期
    #[func] fn connect_to_server(addr: GString) -> GodotResult
    #[func] fn disconnect()
    #[func] fn tick(delta: f64)              // 每帧调用: poll网络+预测+状态更新

    // 状态查询 (GDScript 直接调用)
    #[func] fn get_player_pos() -> Vector2i
    #[func] fn get_visible_tiles() -> Array<Dictionary>
    #[func] fn get_entities() -> Array<Dictionary>
    #[func] fn get_items_ground() -> Array<Dictionary>
    #[func] fn get_message_log() -> Array<Dictionary>
    #[func] fn get_player_hp() -> i32
    #[func] fn get_player_stamina() -> i32
    #[func] fn get_player_hunger() -> i32
    #[func] fn get_player_thirst() -> i32

    // 输入发送
    #[func] fn send_move(direction: i32)    // 0-7 八方向
    #[func] fn send_attack(target_id: i32)
    #[func] fn send_pickup(pos: Vector2i)
    #[func] fn send_wait()
    #[func] fn send_chat(text: GString)
}
```

### GDScript 最终形态

```gdscript
# game_manager.gd — ~40 行
extends Node

@onready var tilemap: TileMap = $Map/TileMap
@onready var entities: Node2D = $Map/Entities
@onready var camera: Camera2D = $Camera
@onready var hud: Control = $HUD

func _ready():
    if not GameClient:
        push_error("GDExtension not loaded")
        return
    var c = GameClient.new()
    add_child(c)
    c.connect_to_server("127.0.0.1:9876")

func _process(delta):
    if not GameClient: return
    GameClient.tick(delta)
    tilemap.sync(GameClient.get_visible_tiles())
    entities.sync(GameClient.get_entities())
    camera.position = GameClient.get_player_pos()
    hud.update_hp(GameClient.get_player_hp())
    hud.update_stamina(GameClient.get_player_stamina())
    hud.update_log(GameClient.get_message_log())

func _input(event):
    # WASD → send_move
    # 鼠标点击实体 → send_attack
    # E → send_pickup
    # Tab → 打开背包
```

---

## 阶段 5：核心玩法系统

| 步骤 | 模块 | 说明 |
|------|------|------|
| 5.1 | `fov.rs` | 递归阴影投射算法 |
| 5.2 | `fov.rs` | 探索遮罩(explored + visible) |
| 5.3 | `ai.rs` | 状态机: Idle → Alert → Chase → Attack |
| 5.4 | `pathfinding.rs` | A* 寻路 |
| 5.5 | `combat.rs` | 命中率 + 伤害计算 + 护甲 |
| 5.6 | `item.rs` | 物品定义 (JSON) + 背包系统 |
| 5.7 | `messagelog.rs` | 带颜色/回合数的日志 |

---

## 阶段 6：高级功能

| 步骤 | 说明 |
|------|------|
| 6.1 | 跨子世界玩家转移（无缝） |
| 6.2 | 同一子世界内多人可见+交互 |
| 6.3 | 合成系统 (Recipe + CraftingManager) |
| 6.4 | 存档/读档 (serde + bincode) |
| 6.5 | Godot UI: 背包界面、合成界面、状态面板 |

---

## 编译验证策略

```
cargo build -p protocol          # 纯 serde，必须零警告
cargo build -p server            # tokio+quinn，必须编译
cargo build -p client --lib      # godot crate 编译 (需要 Godot 头文件)
cargo run   -p server            # 启动服务器监听
```

### 分层测试

```
Layer 1: cargo test -p protocol           # 消息序列化/反序列化 roundtrip
Layer 2: cargo test -p server --lib       # 子世界逻辑单元测试
Layer 3: 独立 client 二进制 → server 联调  # QUIC 网络层
Layer 4: Godot → GDExtension → server     # 完整链路
```

---

## 时间线估计

| 阶段 | 内容 | 估计工作量 |
|------|------|-----------|
| 0 | 项目骨架 | ~1 小时 |
| 1 | 网络层调通 | ~2-3 小时 |
| 2 | 子世界系统 | ~4-6 小时 |
| 3 | 客户端状态管理 | ~3-4 小时 |
| 4 | Godot 集成 | ~3-4 小时 |
| 5 | 核心玩法 | ~8-12 小时 |
| 6 | 高级功能 | ~8+ 小时 |

---

## 关键约束清单

| 领域 | 约束 |
|------|------|
| Rust 版本 | 1.95.0 |
| 网络 | QUIC (quinn 0.11) over UDP :9876 |
| 序列化 | rmp-serde (MessagePack) |
| 帧格式 | `[u32 大端长度][payload]` |
| 异步 | tokio (server + sub_world 全异步) |
| Godot | 4.4+, GDExtension |
| 客户端 Rust | `godot` crate (GDExtension, cdylib) |
| 子世界 | 512×512 tiles, tokio::spawn select! loop |
| 客户端角色 | 反序列化 + 缓存 + 预测 + 暴露高等级 API |
| 掉线处理 | 角色原地等待, 无敌 |
| 证书 | 开发期自签名 + SkipVerify |
