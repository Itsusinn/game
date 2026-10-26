# Dungeon 3D — Godot + Rust

桌面优先的第一人称多人方块游戏。Godot **Forward+ + SDFGI** 负责画面、场景、输入和 UI；Rust 通过 GDExtension 保留模拟、客户端预测和经过证书验证的 Quinn/QUIC。独立 Rust 服务端使用 hecs，不依赖 Godot。

客户端统一使用 Godot，旧桌面入口和自研渲染器已移除。尚待完整实机键鼠及窗口交互验收，架构与验收状态见 [PLAN.md](PLAN.md)。

## 运行

使用 `rust-toolchain.toml` 指定的 stable 工具链、`just`、Godot **4.7.2**。Rust 绑定固定为 `godot = 0.5.5`，使用 `api-4-7`。在 PowerShell 设置 `GODOT_BIN` 为 Godot 可执行文件路径；使用 .NET 编辑器时还需可用的 .NET SDK（本机为 8.0.427）。游戏本身不使用 C#。在仓库根目录分别启动：

```sh
just server
just client
```

客户端启动后显示主界面：**单人游戏、多人游戏、设置、退出游戏**。单人游戏自动在后台启动内置 Rust 权威服务端，通过回环地址的随机端口和经过证书验证的 QUIC 连接；无需单独运行 `just server`，支持现有战斗、敌人、拾取和背包。多人游戏页填写地址、昵称和服务端当前证书，支持同服最多 16 人。

`just client` 会先编译并安装扩展 DLL；重新编译前关闭使用该 DLL 的游戏或编辑器。直接联机可执行 `powershell -File scripts/godot.ps1 run --connect`，直接进入单人游戏使用 `powershell -File scripts/godot.ps1 run --singleplayer`。

设置包含视野、鼠标灵敏度、全局光照和全屏，保存在 Godot 用户目录的 `settings.cfg`。菜单保留 [Fluent Design 的光线、深度、动效和材质思路](https://gameinstitute.qq.com/article/10027)：圆角玻璃按钮、受光边缘、柔和投影、悬停高光与抬升、按压回落及页面缩放淡入。背景采用项目自有的工业场景与方块角色 SVG，整体使用黑白灰、蓝色多人入口及橙色强调。游戏内菜单模糊当前世界，设置、HUD 和行囊共用 `godot/src/interface_theme.gd` 主题。

```sh
just demo
```

`--demo` 是可移动、可跳跃的离线三维漫游场景，用同一套碰撞规则；其中的角色是静态展示。战斗、拾取、聊天和其他玩家需要连接服务器。

## 操作

先单击世界区域捕获鼠标，按 Esc 打开游戏菜单并释放鼠标。菜单提供继续、设置、返回主界面、退出游戏；再次按 Esc 可继续。失去窗口焦点或打开背包时自动释放并清空按键。菜单打开时世界仍在运行，单人和多人均不暂停模拟。

| 输入 | 动作 |
|---|---|
| 鼠标 | 第一人称转向、抬头与低头 |
| WASD / 方向键 | 相对朝向连续移动 |
| Space | 跳跃；需要落地后才能再次起跳 |
| 左键 / F | 面向目标的近战攻击 |
| G | 拾取附近物品 |
| Tab | 打开 / 关闭背包、日志和聊天 |
| Esc | 打开游戏菜单 / 返回上一级 / 继续游戏 |
| 滚轮 / 设置页滑块 | 调整视野角度 |
| 聊天框 + Enter | 发送聊天 |

菜单和聊天不触发移动。联机世界不会因为本地打开菜单而暂停。受击生命值归零后在空闲出生点复活，当前携带物品保留。

## 当前 3D 范围

- 64×32×64 方块世界，Y 轴向上，按 16×16×16 区块传输。世界有边界，不是无限生成世界。
- 起伏地形、草地、树木、石屋遗迹、窗口、屋顶、可跳上的台阶和平台，以及有实体池底的浅水池。未实现游泳。
- 透视相机、深度遮挡、隐藏方块面剔除、程序化像素材质、太阳阴影、SDFGI、SSAO 和距离雾。地形按区块生成网格，只在地图版本变化时重建；动态角色独立更新。SDFGI 替代原 WGSL GI，并非原算法的逐像素复现；动态实体接受 GI，但不承诺像静态地形一样参与 SDFGI 遮挡。
- 连续移动与重力在客户端、服务端共用 30 Hz 固定步长规则，处理地面、墙体、头顶、实体碰撞和未知区域阻挡。
- 第一人称相机按渲染帧率在相邻两次已完成的本地运动状态间插值，消除 30 Hz 移动的阶梯感；位置显示延迟一个模拟步（约 33 毫秒），鼠标转向即时响应。权威位置纠正、复活和切换会话时重置插值。
- 服务端权威近战、敌人追踪和攻击、拾取、背包、聊天、复活和多人同步。敌人使用直接追踪和视线判定，尚无跨高度的完整寻路。
- 按确认范围，本轮不包含挖掘、放置方块、合成、动态流体、昼夜系统或完整账号存档。原先的饥饿、口渴等静态 HUD 已移除，避免展示尚未接通的玩法。

## 网络与存档

- 默认监听 `127.0.0.1:9876`；局域网可设置 `CDDA_BIND=0.0.0.0:9876`。
- 服务端每次启动生成 TLS 证书，默认写入 `save/server.cert.der`，可用 `CDDA_CERT_PATH` 改路径。客户端必须使用服务端本次生成的证书，不跳过验证；远程玩家通过 `--cert FILE.der` 指定证书。
- 协议版本为 **3**，增加三维区块、浮点坐标、速度、落地状态和连续控制输入。旧版客户端不能加入。
- `World3d.seq` 是该玩家最后**执行**的输入序号。客户端只有在有界发送队列接受输入后才预测；收到快照先应用权威位置与速度，再重放未确认输入。服务端每 tick 最多消费每人一条输入，拒绝无效数值、乱序和超量积压。
- 慢客户端的队列不能阻塞广播；网络读取任务持续拥有接收流，不会被 UI 的发送取消。客户端每帧处理网络消息数量有上限。
- 新默认存档为 **`save/world3d.bin`**，以 `DNG3D001` 标记并包含格式版本、尺寸、种子和方块数据。Ctrl+C 正常退出时先写临时文件，再替换目标。
- 原来的 `save/world.bin` **不会自动迁移或覆盖**。把二维文件传给 `CDDA_SAVE_PATH` 会明确报错并停止启动；若要继续保留旧二维地图，先保留原文件，再使用新的三维存档路径。当前没有二维到三维迁移命令。
- 当前仅持久化三维地图；玩家账号、生命、背包、NPC 与物品状态不跨重启保存。加载失败会阻止启动，不能静默生成新世界覆盖存档。

单人模式同样使用 `save/world3d.bin`（相对于启动工作目录），可通过 `CDDA_SAVE_PATH` 指定其他路径。返回主界面、菜单退出和窗口关闭都会先等待后台保存地图、关闭本地服务端并清理网络任务；保存错误会显示在菜单中而不会自动退出。单人存档有文件锁，避免两个内置服务端同时使用同一文件；不要让独立服务端与单人模式同时使用同一个存档。内置证书只在内存中传递，不覆盖独立服务端的证书文件。

客户端参数：`--server IP:PORT`、`--cert FILE.der`、`--name NAME`、`--connect`、`--singleplayer`、`--demo`、`--capture FILE.png`、`--capture-frames 1..4096`（默认 64）、`--lighting gi|direct`（默认 gi）、`--fov 55..100`（默认 72°）、`--yaw DEGREES`。直接运行 Godot 或导出程序时，游戏参数放在 `--` 后；通过 `scripts/godot.ps1` 时由脚本添加。相对证书路径相对于进程工作目录，远程/导出运行建议使用绝对路径。

`--capture` 配合 `--demo`、`--connect` 或 `--singleplayer` 时，在完整区块数据和玩家状态到达后累计指定帧数，让光照历史收敛，再截图并退出；整体 120 秒未完成则报错。未指定游戏模式时截取主界面。设置页可开关 SDFGI；关闭后仍使用 Forward+，不是切换到兼容渲染器。`CDDA_RUN_SECONDS` 可使测试服务器定时正常退出。

## 模块边界

| Crate / 目录 | 职责 |
|---|---|
| `protocol/` | 三维消息、区块与角色共享数据、协议版本 |
| `simulation/` (`game-simulation`) | 平台无关的体素网格查询、固定步长运动、AABB 碰撞、视线检测；由服务端和客户端共用 |
| `client-core/voxel` | 三维地图缓存、输入队列、预测与权威状态协调 |
| `client/` | 会话、QUIC 适配器、固定步长调度与显示插值、纯数据网格生成 |
| `godot-extension/` (`game-godot`) | 主线程桥接：输入、会话、快照、网格批量传递；不重复实现玩法 |
| `godot/` | Forward+ 场景、相机、材质、输入、Godot Control UI 与导出设置 |
| `server/voxel` | 权威 3D 世界、地形生成、hecs NPC、战斗、快照与带版本的地图存储 |

旧二维世界、存档读取和客户端状态模块已移除，当前只保留三维实现；旧存档文件仍按上述策略保留，不提供自动迁移。浏览器仍没有窗口宿主与网络适配器；WASM 检查通过不代表浏览器游戏可运行。

## 验证

执行 `just` 查看带中文说明的全部命令。常用维护命令：

| 命令 | 用途 |
|---|---|
| `just fmt` | 格式化 Rust 代码 |
| `just fmt-check` | 只检查格式 |
| `just clippy` | 严格静态检查 |
| `just test-multiplayer` | 单独运行真实 QUIC 多客户端集成测试 |
| `just doc` | 生成工作区 API 文档 |
| `just clean` | 清理 Cargo 构建缓存，不清理 Godot 缓存或已安装的扩展 DLL |

```sh
just check
just test
just build
just web-check
just godot-smoke
just godot-multiplayer
just export-debug
just export-release
powershell -File scripts/godot.ps1 demo --capture C:/absolute/path/demo.png
```

`just check` 包含格式和严格 Clippy；大型 Cargo 操作使用 `--jobs 2` 串行运行。WASM 检查仅覆盖协议、核心与共享模拟，需要安装 `wasm32-unknown-unknown` target，不包含原生扩展或桌面 QUIC。

Windows 的 just 配方已配置 PowerShell。GPU 截图需要实际图形会话。`godot-smoke` 验证真实场景与扩展，`godot-multiplayer` 启动隔离服务端并验证两个 Godot 桥接实例的真实 QUIC 会话，不能替代两窗口的键鼠操作验收。

导出需安装 Godot 4.7.2 **标准版** Windows x86_64 模板到 `%APPDATA%/Godot/export_templates/4.7.2.stable/`。脚本将模板暂存至 `target/godot-templates/`，从运行时源文件创建独立 `target/godot-export-stage-*/` 项目，并移除其中的编辑器插件和 .NET 配置，再导出不依赖 .NET 的客户端。产物分别位于 Cargo target 目录的 `export-debug/` 和 `export-release/`，发布需一起携带 `Dungeon.exe`、`Dungeon.pck`、`game_godot.dll`，不得混用 debug/release DLL。`just editor` 打开仓库内正式项目。修改 DLL 或运行会导入同一项目的检查前，关闭现有项目编辑器，避免 Windows DLL 占用。

Godot MCP 是可选编辑器工具，已接入本仓库 `godot/`，本地端点为 `http://localhost:17860/mcp`。依赖 Godot 4.7.2 .NET、.NET SDK 8 与 `godot-cli@0.33.0`。`Dungeon.Editor.csproj` 仅编译编辑器插件，游戏仍使用 Rust/GDScript；插件源码和生成的代理技能被 Git 忽略。首次运行先 `just build`，再执行 `powershell -File scripts/godot-mcp.ps1` 安装固定版本插件、构建并启动无窗口编辑器。已打开时先用 `godot-cli close godot --owned-only` 关闭，再更新 DLL 或插件程序集。

操作前通过 MCP 查询 `editor | select projectPath, editedScene`，必须指向本仓库，不能把 `GodotMcpSandbox` 当验收项目。修改后用 `editor | call SaveScene()` 保存。2026-10-27 已通过 MCP 给主场景写入职责说明、保存并回读，主脚本验证无错误。导出排除插件、C#、测试、代理技能和工具配置；运行游戏无需 MCP 服务或编辑器。安装阶段的旧开始菜单快捷方式仍指向沙盒，应使用本仓库脚本启动正式项目。

真实 QUIC 集成测试位于 `client/tests/multiplayer.rs`，覆盖两客户端区块传输、连续移动和跳跃、逐玩家输入确认、聊天、断开释放与旧协议拒绝。验证运行使用独立端口、`target/` 下的存档和证书，不能用真实 `save/` 做破坏性测试。
