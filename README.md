# Dungeon — Rust / WebGPU

桌面优先的 2D 多人地牢游戏。使用 Rust + wgpu/WGSL 自研渲染器、winit 窗口与输入、egui 界面、hecs ECS 和 Quinn/QUIC。无需安装 Godot。

## 运行

从仓库根目录运行，使用当前 Rust stable 工具链：

```sh
cargo run -p server
```

再打开一个终端：

```sh
cargo run -p client -- --connect
```

也可以不传 `--connect`，在连接菜单中填写地址、名字和证书路径。启动多个客户端即可进行本机多人联机。

```sh
cargo run -p client -- --demo
```

`--demo` 是独立的渲染展示，不启动模拟或服务器。退出展示后连接服务器即可游玩。

## 操作

| 输入 | 动作 |
|---|---|
| WASD / 方向键 | 移动 |
| Q / E / Z / C，数字键盘 | 斜向移动 |
| Space / `.` | 等待 |
| F | 攻击相邻敌人 |
| G | 拾取脚下物品 |
| Tab | 展开或收起背包 |
| 滚轮 / 侧栏滑块 | 缩放 |
| 聊天输入框 + Enter | 同区域聊天 |

输入框获得焦点时，打字不会触发移动。服务器继续保留原有 200ms AI 更新节奏；它不是所有玩家共同等待的严格回合制。

## 网络和存档

- 默认监听 `127.0.0.1:9876`。局域网使用环境变量 `CDDA_BIND=0.0.0.0:9876`，客户端填写服务器的局域网 IP。
- 服务器每次启动生成自签名 TLS 证书，写入 `save/server.cert.der`。客户端使用这个文件验证服务器，**不跳过证书校验**。远程玩家需要取得服务器本次启动的证书文件，使用 `--cert PATH`。
- `CDDA_CERT_PATH` 可调整证书输出位置；证书里的 `localhost` 是固定 TLS 名称，连接地址独立指定。
- `CDDA_SAVE_PATH` 默认为 `save/world.bin`。Ctrl+C 正常退出会保存；损坏的现有存档会阻止启动，避免静默覆盖。
- 原有 MessagePack 地图存档格式保持兼容，现在会恢复地图及探索状态，并保留未访问区域。NPC、地面物品仍按种子生成；玩家账号、背包和战斗状态**尚不跨服务器重启持久化**。跨区域转移会保留当前玩家状态。
- 协议版本为 2，增加最大生命值和背包字段，旧客户端不能连接。

客户端参数：`--server IP:PORT`、`--cert FILE.der`、`--name NAME`、`--connect`、`--demo`、`--capture FILE.png`。截图模式在世界数据到达后保存一帧并退出；30 秒内未获得世界数据则失败。`CDDA_RUN_SECONDS` 可让服务器定时正常退出，供自动验证使用。

## 模块边界

| Crate | 职责 |
|---|---|
| `protocol` | 消息类型、坐标、协议版本 |
| `game-client-core`（`client-core/`） | 平台无关的状态、探索缓存、输入队列、预测与确认后重放 |
| `game-engine`（`engine/`） | 相机、视口裁剪判定、实例化图元、WGSL 管线和 GPU 缓冲区 |
| `client` | 桌面宿主、egui UI、输入映射、QUIC 传输适配器和画面组装 |
| `server` | 权威模拟、hecs 组件、AI/FOV、分区管理、QUIC 会话、地图存储 |

`engine` 不依赖窗口、网络或游戏协议；`client-core` 不依赖 GPU、Tokio 或桌面 API。未来浏览器宿主可复用两者，但仍需实现浏览器事件循环和 WebTransport/WebSocket 适配，当前没有可直接部署的浏览器客户端。

## 验证

```sh
cargo fmt --all -- --check
cargo check --workspace --all-targets --jobs 2
cargo test --workspace --jobs 2
cargo clippy --workspace --all-targets --jobs 2 -- -D warnings
cargo check -p game-client-core -p game-engine --target wasm32-unknown-unknown --jobs 2
cargo run -p client -- --demo --capture target/demo.png
```

跨平台检查需要先安装 `wasm32-unknown-unknown` target。图形截图需要真实图形会话和可用 GPU，常规单元测试与网络集成测试不依赖 GPU。

安装了 `just` 时可使用 `just server`、`just client`、`just demo`、`just check`、`just test`、`just build`。

## 当前范围

已接通移动、近战、拾取与背包显示、聊天、多人可见、区域转移、地图存储和加载。原项目中的远程攻击、物品使用/丢弃、合成等协议枚举或辅助模块仍未形成完整玩法，不在 UI 中提供虚假的操作入口。美术目前使用程序化图元，尚无贴图资产管线、音频系统或通用编辑器。
