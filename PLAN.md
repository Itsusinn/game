# Godot + Rust 迁移架构与进度

## 已确认方向

用户已明确将二维地牢改成 Minecraft 式的完整 3D 可玩基础：第一人称、三维方块世界、连续移动、跳跃、碰撞、战斗和联机。本轮不包含挖掘建造。

已确认使用 Godot + Rust，并选择 **Forward+ + SDFGI**。Godot 负责窗口、输入、场景、画面和 UI，Rust 通过 godot-rust/gdext 接入。保留 Rust stable、hecs 和经过 TLS 验证的 Quinn。首版世界为 64×32×64，有固定边界；暂不增加无限区块流送或多区域迁移。

版本固定为 Godot 4.7.2、`godot = 0.5.5` / `api-4-7`。正式项目为 `godot/`，桥接 crate 为 `godot-extension/`。服务端及 `protocol/`、`simulation/`、`client-core/` 不依赖 Godot；网络后台任务与场景树隔离。

## 迁移阶段（2026-10-27）

1. 已记录原有未提交移动插值修改，迁移到 `client/src/movement.rs` 并保留六项行为测试。原客户端截图在忽略目录 `target/migration-baseline/legacy.png`；原 workspace 测试通过。
2. 已建立真实 GDExtension、主线程批量桥接和可复用会话层。证书读取移到网络线程，断线用取消信号清理，不在 Godot 主线程 join。协议、模拟和存档格式未变。
3. 已接通分区块地形、角色和物品网格、程序化材质、相机、输入、固定步长和插值；离线 headless 场景测试通过，包括跳跃、碰撞、输入边界、焦点释放、Tab、FOV 和会话切换。
4. 已接通真实 QUIC、菜单、HUD、背包、日志、聊天及攻击/拾取命令。Godot 内两个桥接实例测试通过：连接、移动/跳跃及同步、拾取、聊天、近战、断开和重新连接。Rust 测试覆盖敌人攻击、死亡复活保留背包、证书失败、协议及存档保护。两独立窗口的交互验收仍待完成。
5. Windows debug/release 均已导出，独立 EXE 在 RX 7900 XTX 上运行 Forward+。两份导出程序同时连接隔离的独立 Rust 服务端，接收世界、完成 GPU 截图、正常退出，服务端正常保存；日志和截图位于 `target/export-network-b4b381eb6e024a8f94d7f8b6a6ff474b/`。没有启动本项目编辑器，标准模板不依赖 .NET 或 MCP；外部安装验证沙盒未参与。`just check`、`cargo test --workspace --jobs 2`、`just build`、`just web-check`、`just godot-smoke` 和 `just godot-multiplayer` 通过。MSVC 链接器有创建导入库的提示，严格 Clippy 无警告。

尚未完成：实际键鼠、DPI/窗口缩放/最小化、两窗口联机与完整视觉对照。Computer Use 对 Godot .NET 的授权被拒绝，未绕过；BotBus 图片分享调用也被拒绝。保留旧 `engine/` 和显式 `legacy` 客户端作为临时参考，待替代路径完成实机验收后再移除。这一状态不能称为整个重构已完成。

## MCP 正式项目接入（2026-10-27）

- 已关闭 CLI 管理的安装验证沙盒编辑器，将 `http://localhost:17860/mcp` 接到 `C:/Workspace/Code/game/godot/`。
- Godot-MCP 插件版本 0.33.0，编辑器程序集 `Dungeon.Editor.csproj`；仅编辑器工具使用 .NET，运行时 Rust/GDScript 边界不变。可复现启动命令见 README。
- 经 MCP `rql_mutate` 设置主场景根节点 `EditorDescription` 并调用 `SaveScene()`；MCP 回读与本地 `scenes/main.tscn` 一致。`script_validate` 检查 `src/main.gd` 返回零错误、零警告。本次修改不改变玩法或画面，不能替代前述实机交互验收。
- 导出脚本改用纯运行时暂存项目，避免插件及其项目配置进入产物，并隔离 MCP 编辑器占用的 DLL。重新导出 release 后检查 PCK 未包含工具名称、工具配置或凭据路径；独立客户端加载 Rust 扩展、启动离线场景并正常退出。

## 数据流

```text
Godot InputMap / 鼠标 / UI
    -> GameBridge（Godot 主线程）
    -> 30 Hz PlayerInput
    -> 有界 QUIC 发送队列成功入队
    -> Client3d 待确认队列 + game-simulation 本地预测
    -> 服务端每 tick 最多执行每玩家一条输入
    -> VoxelWorld 权威运动 / hecs NPC / 战斗 / 拾取
    -> World3d（执行确认号、Body、角色与物品）
    -> 客户端应用权威状态并重放未确认输入
    -> 批量状态快照、纯数据区块/实体网格
    -> Godot ArrayMesh / Camera3D / Forward+ + SDFGI / Control UI
```

`game-simulation` 是新增的无平台依赖共享模块，拥有网格查询和运动规则，避免服务端依赖客户端实现，也避免把物理逻辑塞进协议层。Y 轴向上，X/Z 为水平面；Body 使用连续浮点坐标和速度。输入不携带时间步长或最终坐标，客户端不能用超大 dt 或位置直接驱动权威状态。

## 世界与网络

- 固定 30 Hz 模拟，运动、重力、跳跃、体素与实体碰撞共用实现。未知体素和世界边界阻挡运动。角色间使用有高度的 AABB，避免穿过其他角色。
- 区块边长 16；登录通过有序可靠流发送 32 个区块，客户端完整接收后启用控制。静态地图无需每次快照重复传输。
- 每玩家输入队列有上限；非法数值、超量队列、乱序输入被拒绝。`seq` 只确认已执行输入，不能用世界 tick 替代。
- NPC 由 hecs 管理，协议 ID 独立于 ECS 句柄；玩家与 NPC 的 ID 空间分离。
- 近战必须满足距离、朝向、视线和攻击间隔。玩家 HP 归零会寻找空闲出生点复活。NPC 直接追踪，不承诺完整三维寻路。
- TLS 继续使用当前服务器生成的证书；持续读流、有界发送和非阻塞广播保留。3D 协议版本为 3，与旧客户端不兼容。

## 渲染与窗口

- `client::presentation` 生成纯 CPU 网格，剔除相邻实心方块之间的面；每个 16³ 区块对应一个 Godot 网格节点，没有逐方块节点。当前地图不可编辑，仅地图版本变化时重建静态地形，动态实体按 ID 复用节点；后续加入可编辑地形时需进一步按脏区块更新。
- Godot 主线程创建 ArrayMesh、资源和节点；后台仅传递 Rust 数据。Godot 不再移动同一玩家的物理刚体，碰撞完全由 Rust 共享模拟负责。
- 使用 Forward+、SDFGI、SSAO、太阳阴影、程序化像素材质与距离雾。原 WGSL/kajiya 思路的光照是视觉参考，没有嵌入旧 wgpu 渲染循环。SDFGI 与原算法效果不完全相同，动态遮挡和细节表现须实机比较。UI 的 GI 开关只关闭 SDFGI，不切换渲染器。
- Godot 管理窗口、渲染资源和 Control UI；缩放、DPI、最小化与鼠标交互仍须实际图形会话验收。
- 点击世界捕获鼠标，Esc / 失焦 / 打开背包释放。松键不受 UI 消费事件影响，防止按键卡住。
- 旧 egui 代码在删除前继续遵守多次布局仅收集命令、布局后执行一次副作用的约束；Godot 主入口使用 Control 信号处理 UI 命令。

## 存档策略

默认写 `save/world3d.bin`，使用文件标记 `DNG3D001`、格式版本 1、固定尺寸、种子和方块数组；写临时文件并刷盘后替换。格式、尺寸、数组长度和方块类型均在加载时验证。

旧 `save/world.bin` 不自动迁移、不覆盖。向 3D 入口提供旧文件或损坏文件会失败并停止启动。本轮不提供二维地图转换器。旧二维模型、存档读取模块及对应协议已移除；当前程序仅使用 VoxelWorld / Client3d。需要历史实现时可从 Git 历史查看。

当前仍只持久化地图，角色、NPC、背包和地面物品不是持久化账号进度。所有运行验证使用隔离路径与端口。

运行方式、限制和验证命令见 README.md。
