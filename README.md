# Dungeon 3D — Godot + Rust

桌面优先的第一人称多人方块游戏。Godot **Forward+ + SDFGI** 负责画面、场景、输入和 UI；Rust 通过 GDExtension 保留模拟、客户端预测和经过证书验证的 Quinn/QUIC。独立 Rust 服务端使用 hecs，不依赖 Godot。

客户端统一使用 Godot，旧桌面入口和自研渲染器已移除。尚待完整实机键鼠及窗口交互验收，架构与验收状态见 [PLAN.md](PLAN.md)。

## 运行

使用 `rust-toolchain.toml` 指定的 stable 工具链、`just`、Godot **4.7.2**。Rust 绑定固定为 `godot = 0.5.5`，使用 `api-4-7`。在 PowerShell 设置 `GODOT_BIN` 为 Godot 可执行文件路径；使用 .NET 编辑器时还需可用的 .NET SDK（本机为 8.0.427）。游戏本身不使用 C#。在仓库根目录分别启动：

```sh
just server
just client
```

也可一条命令启动独立服务端并让客户端自动连接：

```sh
just dev
```

`just dev` 先构建，等待本次服务端写好证书并开始监听，再启动客户端；关闭客户端后，服务端正常保存并退出。支持 `CDDA_BIND`（默认 `127.0.0.1:9876`，端口不能为 0）、`CDDA_SAVE_PATH`、`CDDA_CERT_PATH` 和 `CDDA_IDENTITY_PATH`。绑定 `0.0.0.0` 或 `::` 时，本机客户端使用对应回环地址连接。启动失败会报错，不会连接已有的其他服务端。该 Windows 启动器为 `scripts/dev.rs` Cargo script，需要额外安装 `rustup toolchain install nightly --profile minimal`；游戏本身仍用仓库的 stable 工具链构建。终端显示两端日志，启动器强制 `server=info` 以确认就绪；Ctrl+C 会关闭客户端并通知服务端保存。

客户端启动后显示主界面：**单人游戏、多人游戏、设置、退出游戏**。单人游戏自动在后台启动内置 Rust 权威服务端，通过回环地址的随机端口和经过证书验证的 QUIC 连接；无需单独运行 `just server`，支持现有战斗、敌人、拾取和背包。多人游戏页填写地址、昵称和服务端当前证书，支持同服最多 16 人。

`just client` 会先编译并安装扩展 DLL；重新编译前关闭使用该 DLL 的游戏或编辑器。直接联机可执行 `powershell -File scripts/godot.ps1 run --connect`，直接进入单人游戏使用 `powershell -File scripts/godot.ps1 run --singleplayer`。

设置包含视野、鼠标灵敏度、全局光照和全屏，保存在 Godot 用户目录的 `settings.cfg`。视觉采用自然写实方向：石材、旧木板、树皮与草地 PBR 表面，柔和日光；UI 使用炭灰与灰绿色、细边框、小圆角和轻微反馈。菜单背景来自项目场景实机渲染，游戏内菜单模糊当前世界；设置、HUD、背包与快捷栏共用 `godot/src/interface_theme.gd` 主题。

```sh
just demo
```

`--demo` 是可移动、可跳跃的离线三维漫游场景，用同一套碰撞规则；其中的角色是静态展示。战斗、拾取、聊天和其他玩家需要连接服务器。

## 操作

进入游戏后自动捕获鼠标，按 Esc 打开游戏菜单并释放鼠标。菜单提供继续、设置、返回主界面、退出游戏；再次按 Esc 可继续。背包、箱子、聊天等 UI 打开时释放鼠标，关闭后自动恢复捕获。失去窗口焦点时释放鼠标并清空按键，重新聚焦且没有 UI 打开时自动捕获。菜单打开时世界仍在运行，单人和多人均不暂停模拟。

| 输入 | 动作 |
|---|---|
| 鼠标 | 第一人称转向、抬头与低头 |
| WASD / 方向键 | 相对朝向连续移动 |
| Space | 跳跃；需要落地后才能再次起跳 |
| 左键 | 破坏准星指向的方块，收入物品栏 |
| 右键 | 在目标表面放置当前快捷栏方块，消耗 1 个 |
| F | 面向目标的近战攻击 |
| G | 拾取附近物品 |
| E | 准星对准附近箱子时打开，Esc 关闭 |
| Tab | 打开 / 关闭背包、日志和聊天 |
| 1–9 / 滚轮 | 选择 / 循环切换快捷栏槽位 |
| 物品栏左键 / 拖动 | 移动、合并同类物品，整组不同物品交换槽位 |
| 物品栏右键 | 选中半组；已有选择时向目标放入 1 个 |
| Shift + 点击 | 背包与快捷栏快速转移；打开箱子时快速存取 |
| Esc | 打开游戏菜单 / 返回上一级 / 继续游戏 |
| 设置页视野滑块 | 调整视野角度 |
| 聊天框 + Enter | 发送聊天 |
| / | 打开聊天并预填指令前缀 |
| 飞行时 Space / Shift / Ctrl | 上升 / 下降 / 3 倍加速，松键悬停 |

菜单和聊天不触发移动。联机世界不会因为本地打开菜单而暂停。受击生命值归零后在空闲出生点复活，当前携带物品保留。

### 玩家指令

单人和多人中的所有玩家均可在聊天框输入指令，按 Enter 执行；按 `/` 可直接打开输入框，Esc 返回游戏。指令由服务端执行，反馈显示在自己的行动记录中，不广播给其他玩家；普通聊天仍共享。离线 `--demo` 展示不支持指令。

| 指令 | 效果 |
|---|---|
| `/help` | 查看指令帮助 |
| `/tp x y z` | 传送自己到脚底坐标，如 `/tp 28.5 20 32.5` |
| `/tp ~ ~5 ~` | 相对当前位置向上移动 5 米；各轴支持 `~` 或 `~偏移` |
| `/fly` | 切换自己的飞行状态 |
| `/fly on` / `/fly off` | 开启 / 关闭飞行 |

飞行保留方块、角色、未知区块与世界边界碰撞，WASD 水平移动，Space 上升、Shift 下降、Ctrl 加速至 3 倍；松键悬停。HUD 标记当前飞行状态。关闭后恢复重力；死亡复活、断线重连和重启后默认关闭，飞行开关不写入存档。传送位置沿用现有玩家存档保存，不改变存档格式。

传送会生成目标附近地形并分批同步、卸载远处区块，拒绝非有限数字、越界、方块或角色占用的落点；等待目标附近区块时暂停本地预测，权威位置到达后重置画面插值。改变状态的指令每人间隔至少 0.5 秒，传送落点检查失败也计入间隔。

## 当前 3D 范围

- 按需扩展的方块世界，X/Z 支持正负坐标，Y 高度保持 32，按 16×16×16 分段传输。初始 64×32×64 区域保留原场景；探索时生成新区块。水平技术范围为 `[-32768, 32768)`，用于控制 f32 物理与渲染精度，垂直范围为 `[0, 32)`。
- 起伏地形、草地、树木、石屋遗迹、窗口、屋顶、可跳上的台阶和平台，以及有实体池底的浅水池。未实现游泳。
- 透视相机、深度遮挡、隐藏方块面剔除、PBR 材质、太阳阴影、SDFGI、SSAO、ACES 色调映射和距离雾。地形按区块生成网格，只在地图版本变化时重建；动态角色独立更新。保留现有方块几何与角色造型，写实化主要体现在材质、光照和 UI。SDFGI 替代原 WGSL GI，并非原算法的逐像素复现；动态实体接受 GI，但不承诺像静态地形一样参与 SDFGI 遮挡。
- 连续移动与重力在客户端、服务端共用 30 Hz 固定步长规则，使用 Rapier 3D 0.36.1 角色控制器处理地面、墙体、头顶、实体碰撞和未知区域阻挡。
- 第一人称相机按渲染帧率在相邻两次已完成的本地运动状态间插值，消除 30 Hz 移动的阶梯感；位置显示延迟一个模拟步（约 33 毫秒），鼠标转向即时响应。权威位置纠正、复活和切换会话时重置插值。
- 服务端权威近战、敌人追踪和攻击、拾取、背包、聊天、复活和多人同步。敌人使用直接追踪和视线判定，尚无跨高度的完整寻路。
- 玩家物品栏为 27 格背包 + 9 格快捷栏，每格最多 64 个；快捷栏是物品栏的前 9 格，不复制物品。支持拖放、点击移动、拆分、合并、整组交换和 Shift 快速转移。底部显示快捷栏、数量及当前物品名称，1–9 或滚轮切换；选中的方块可右键放置，暂不增加装备或攻击效果。背包满时拾取物留在地面；Esc 首先取消选中，再关闭界面。
- 箱子是有碰撞的容器方块，新地图在出生点附近 `(26, 7, 30)` 生成一个空箱子。准星对准箱子且距中心不超过 3.5 米时按 E 打开，与物品栏共用格子界面，支持跨区域拖放、单个或整组存取（界面每次最多 64）。为兼容既有存档，箱子仍容纳 24 类物品，每类最多 999 个；同类自动归并，空类移除。多人共享，由服务端检查距离、遮挡、数量、箱子版本和背包版本，失败不改变任何一端。离开范围、被遮挡、死亡、失焦或断线会关闭箱子；打开界面时世界仍运行。
- 单人及多人支持生存式方块编辑：每次点击破坏或放置一个方块，交互距离为视线 4.5 米，服务端成功操作间隔至少 0.2 秒。破坏直接收入对应方块，背包满则拒绝破坏；放置消耗当前快捷栏的 1 个方块。草方块、泥土、石块、原木、树叶、木板和空箱子可操作，水与世界底层不可破坏，水格不能被覆盖。非空箱子必须先取空；破坏空箱子会关闭其他玩家正在查看的该箱子。禁止越界、穿墙、覆盖已有方块或地面物品，以及在玩家/NPC 身体内放置。地形变化更新共享碰撞，并广播变化区块；客户端仅重建该区块及相邻区块网格。编辑后的地图、箱子与玩家物品共同持久化，旧地图无需额外迁移。离线 `--demo` 仍只用于漫游。
- 本轮不包含合成、工具耐久、动态流体、昼夜系统或完整账号认证。原先的饥饿、口渴等静态 HUD 已移除，避免展示尚未接通的玩法。

## 材质与视觉检查

材质位于 `godot/materials/textures/`，使用 Poly Haven 的 CC0 1K JPG 颜色、OpenGL 法线和粗糙度贴图。来源、许可及 SHA-256 校验清单见 [素材说明](godot/materials/textures/LICENSE.md) 和 [sources.json](godot/materials/textures/sources.json)。纹理沿世界坐标连续铺设，箱子保留局部纹理和金属扣件。材质通过 Godot 导入，正式导出包含全部资源，不需要联网下载。

### 资产名单

| 资产 | 用途 | 仓库文件 | 来源与许可 |
| --- | --- | --- | --- |
| Weathered Brown Planks | 木板、箱子和物品图标 | `godot/materials/textures/weathered_brown_planks_*_1k.jpg` | [Poly Haven](https://polyhaven.com/a/weathered_brown_planks)，CC0 1.0 |
| Stone Brick Wall 001 | 石墙与石材表面 | `godot/materials/textures/stone_brick_wall_001_*_1k.jpg` | [Poly Haven](https://polyhaven.com/a/stone_brick_wall_001)，CC0 1.0 |
| Leafy Grass | 草地；经着色用于泥土与方块树冠 | `godot/materials/textures/leafy_grass_*_1k.jpg` | [Poly Haven](https://polyhaven.com/a/leafy_grass)，CC0 1.0 |
| Bark Brown 02 | 树干表面 | `godot/materials/textures/bark_brown_02_*_1k.jpg` | [Poly Haven](https://polyhaven.com/a/bark_brown_02)，CC0 1.0 |
| 主菜单场景背景 | 当前主菜单背景 | [frontier_background.png](godot/materials/frontier_background.png) | 项目场景自行渲染，使用上述 CC0 材质 |
| 旧版工业场景插画 | 保留的旧版背景，当前菜单未使用 | [frontier_backdrop.svg](godot/src/frontier_backdrop.svg) | 项目自制 SVG |

每套第三方材质包含 `diff`（颜色）、`nor_gl`（OpenGL 法线）和 `rough`（粗糙度）三张 1K JPG，共 12 张原始贴图。下载日期为 2026-10-27，许可见 [Poly Haven 资产许可](https://polyhaven.com/license) 与 [CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/legalcode)。菜单背景可通过 `godot/tests/visuals.gd` 的无 HUD 截图重新生成。

字体通过 Godot `SystemFont` 使用系统中的 Segoe UI、Microsoft YaHei UI 或 Noto Sans CJK SC，仓库未分发这些字体文件。

### 视觉检查

在完成 `just build` 和项目导入后，可用图形窗口生成世界、背包、暂停菜单、主菜单及设置页截图：

```powershell
& $env:GODOT_BIN --path godot --script res://tests/visuals.gd
```

输出默认位于 `target/visual-review/`，可用 `CDDA_VISUAL_PATH` 指定目录。该检查使用离线展示场景，临时启用 GI 和 72° 视野，不读写真实存档或覆盖用户设置；需要实际图形环境，不能用 headless 代替画面验证。

## 网络与存档

- 默认监听 `127.0.0.1:9876`；局域网可设置 `CDDA_BIND=0.0.0.0:9876`。
- 服务端每次启动生成 TLS 证书，默认写入 `save/server.cert.der`，可用 `CDDA_CERT_PATH` 改路径。客户端必须使用服务端本次生成的证书，不跳过验证；远程玩家通过 `--cert FILE.der` 指定证书。
- 协议版本为 **9**，输入增加下降/加速按键、权威快照增加飞行状态；指令复用聊天消息。保留区块列卸载和按位置分批同步、客户端稳定 UUID、恢复镜头朝向及物品栏版本校验。版本 8 及更旧客户端不能加入，客户端与服务端须同时更新。世界存档仍为版本 6。
- `World3d.seq` 是该玩家最后**执行**的输入序号。客户端只有在有界发送队列接受输入后才预测；收到快照先应用权威位置与速度，再重放未确认输入。服务端每 tick 最多消费每人一条输入，拒绝无效数值、乱序和超量积压。
- 慢客户端的队列不能阻塞广播；网络读取任务持续拥有接收流，不会被 UI 的发送取消。客户端每帧处理网络消息数量有上限。
- 默认存档为 **`save/world3d.bin`**，保留 `DNG3D001` 标记，格式版本 **6**。MessagePack 清单保存高度、种子、已生成区块列坐标和归档引用；区块、玩家和生物实体分别位于 `world3d.chunks/`、`world3d.players/`、`world3d.entities/`，文件名均为 `<同一批次 UUID>.rkyv`，由 rkyv + memmap2 读写。先写完并同步三个不可变归档，再原子替换世界清单；保存中断仍加载上一个完整批次，不混用地图、箱子、地面物品、怪物和玩家背包。
- 读取版本 1 地图时验证并补箱，备份为 `world3d.v1.bak`；版本 2 保留箱子，备份为 `world3d.v2.bak`；版本 3 验证原玩家归档，备份为 `world3d.v3.bak` 并保留其引用的 `.players/` 文件；版本 4 验证原区块与玩家归档，备份为 `world3d.v4.bak` 并保留其引用的 `.chunks/`、`.players/` 文件；版本 5 验证全部三类归档后备份为 `world3d.v5.bak`，保留其引用的 `.chunks/`、`.players/`、`.entities/` 文件。已有不同备份时使用编号后缀。补箱优先位置为 `(26, 7, 30)`，被占用则选择安全空位，不覆盖原方块。备份失败或无安全空位则停止加载，原文件不变；下一次成功保存才升级到版本 6。版本 1–4 没有怪物记录，迁移时按原规则初始化三只怪物，此后保存其状态和死亡后的缺席。旧程序不能读取版本 6；回退时恢复对应版本备份至原世界文件名，版本 3/4/5 还需要备份引用的全部归档目录。版本 5 迁移保持地形、箱子、物品、角色和怪物缺席记录不变，原区域不会重新生成。
- 原来的 `save/world.bin` **不会自动迁移或覆盖**。把二维文件传给 `CDDA_SAVE_PATH` 会明确报错并停止启动；若要继续保留旧二维地图，先保留原文件，再使用新的三维存档路径。当前没有二维到三维迁移命令。
- 当前持久化地图、箱内物品、地面物品、怪物，以及所有已访问角色的生命、位置、速度、朝向、背包槽位和选中快捷栏；离线角色也会保留。每 30 秒、玩家断线及服务端正常退出时共同保存。强制终止最多回退到上次成功提交的批次。怪物恢复原位置、运动、朝向、生命、UUID、网络 ID 和攻击冷却；已击杀怪物不会在读档时重新生成。玩家恢复位置被地形或其他实体占用时改用安全出生点，保留背包；玩家死亡状态按原有规则复活。
- 客户端首次连接在 Windows `%APPDATA%/Dungeon3D/client.uuid` 生成随机 UUID，可用 `CDDA_IDENTITY_PATH` 指定独立身份文件。改昵称仍恢复同一角色，同名不同 UUID 相互独立，同一 UUID 同时上线会被拒绝。身份文件损坏会明确报错，不会自动换号；换电脑需复制原身份文件。这是角色标识，不是账号认证。
- 备份版本 6 时必须一起复制世界清单、对应 `.chunks/`、`.players/` 和 `.entities/` 目录；旧批次归档保留，不自动清理。损坏或缺失的归档、区块、分段，以及批次不一致、未知实体类型、重复身份或非法实体字段都会阻止加载，不会重新生成或覆盖原世界。

单人模式同样使用 `save/world3d.bin`（相对于启动工作目录），可通过 `CDDA_SAVE_PATH` 指定其他路径。返回主界面、菜单退出和窗口关闭都会先等待后台共同保存、关闭本地服务端并清理网络任务；保存错误会显示在菜单中而不会自动退出。独立服务端和单人模式使用相同的排他世界锁，阻止同时打开同一存档。内置证书只在内存中传递，不覆盖独立服务端的证书文件。

客户端参数：`--server IP:PORT`、`--cert FILE.der`、`--name NAME`、`--connect`、`--singleplayer`、`--demo`、`--capture FILE.png`、`--capture-frames 1..4096`（默认 64）、`--lighting gi|direct`（默认 gi）、`--fov 55..100`（默认 72°）、`--yaw DEGREES`。直接运行 Godot 或导出程序时，游戏参数放在 `--` 后；通过 `scripts/godot.ps1` 时由脚本添加。相对证书路径相对于进程工作目录，远程/导出运行建议使用绝对路径。

`--capture` 配合 `--demo`、`--connect` 或 `--singleplayer` 时，在完整区块数据和玩家状态到达后累计指定帧数，让光照历史收敛，再截图并退出；整体 120 秒未完成则报错。未指定游戏模式时截取主界面。设置页可开关 SDFGI；关闭后仍使用 Forward+，不是切换到兼容渲染器。`CDDA_RUN_SECONDS` 可使测试服务器定时正常退出。

## 模块边界

### 动态区块同步

参考 [Minecraft 区块同步](https://zh.minecraft.wiki/w/Java版网络协议/区块同步) 的附近加载与显式卸载思路，仍使用本项目的 Quinn/QUIC 和 MessagePack，不实现 Minecraft 协议兼容。

- 服务端以权威玩家所在 X/Z 区块为中心，提前加载半径 3 的 7×7 区块列；每列高 32。优先发送最近区块，每玩家每 tick 最多 8 条加载/卸载消息（最多 4 个新区块列），为快照保留队列空间。客户端附近 3×3 区块列到齐即可进入游戏，无需等待全图。
- 玩家移动导致加载窗口变化时，先显式卸载离开窗口的客户端区块，再发送新范围内的数据。已生成区块直接复用；未生成区块根据种子和世界坐标生成，树冠跨区块查询，不受探索顺序影响。客户端最多缓存 49 列、98 个分段；未知空间继续阻挡移动。
- 方块编辑只通知已收到该区块的玩家；后来进入范围的玩家获得最新完整区块。加载、编辑、卸载均刷新对应网格及相邻面的可见性，Godot 删除卸载网格、复用无变化节点。断线和新会话清空缓存。
- 地形显示使用去重的待更新队列，优先玩家附近，每帧最多重建 2 个分段，目标 CPU 预算 2 毫秒（单个分段不可中断，可能略超预算）。卸载立即删除节点并取消待更新项，切换会话清空队列；没有新网络修订时也继续处理。碰撞数据立即更新，远处网格可稍后显示，自动截图等待队列清空。Rust 网格生成缓存相邻分段引用，顶点属性一次分配后通过切片批量写入 Godot 数组，避免逐顶点引擎调用；不降低视距、材质或 GI 设置。
- 当前服务端将所有已探索区块保留在内存中，每个存档批次仍整体写入稀疏归档；内存和保存成本随探索增长，尚未实现服务端区块驱逐或按区域增量落盘。沿用单归档 64 MiB 的保护限制，超限会明确保存失败并保留上次完整存档。
- `--demo` 仍为固定区域离线展示；单人游戏和多人服务器均使用动态生成。

区块流送的图形回归可执行 `just godot-streaming`：先用真实 QUIC 在隔离平坦跑道上走过 x=64 和 x=0、保存重开，再启动 Forward+ 图形窗口，从保存的负坐标位置继续跨区块移动。验证新区块网格出现、旧节点删除、缓存不超过 98 个分段和退出保存；截图位于输出的 `target/streaming-quic/<UUID>.streaming.png`。该脚本注入移动输入，不代表手动键鼠或长时间探索验收。

`just godot-streaming-perf` 在实际图形窗口、隔离存档中执行近距离/远距离传送和 7 秒加速飞行，记录主线程、地形更新和整帧的 p50/p95/峰值，同时检查每帧更新上限、队列排空、卸载和会话清理。结果与截图位于 `target/streaming-perf-<标签>-<编号>/`；`CDDA_PERF_LABEL` 设置标签，`CDDA_PERF_GI=on|off` 只覆盖本次测试的 SDFGI，不写入用户设置。不指定时使用当前设置；比较前后性能时保持渲染设置相同，避免同时运行其他游戏或构建。短时测试不保证所有场景下的帧时间。

### 区块数据编解码

`server::chunk_data` 参考 [Minecraft Chunk format](https://minecraft.wiki/w/Chunk_format)，采用 `DataVersion`、区块坐标、`Status`、`LastUpdate`、`sections` 和 `block_entities` 的组织方式。本项目按探索范围保存稀疏的 16×16 区块列，每列两个 16³ 分段；这是项目自己的 rkyv 格式，不是 NBT/Anvil 文件，也不提供 Minecraft 地图导入。

每个分段用稳定的 `game:*` 方块名称调色板和 u64 压缩索引保存，索引顺序为 `y*256+z*16+x`。单一方块无需索引数组；其余至少 4 位，索引不跨 u64 边界。箱子作为所属区块的方块实体保存位置、修订号和物品，地面物品同样归属区块列。空分段也显式记录，缺失数据视为损坏。当前没有增加生物群系或光照缓存；未探索区块不写入归档，已生成区块必须与世界清单一致，缺失时拒绝加载。

区块文件使用 `DNGCHK01`、16 字节头、版本 2（兼容读取旧版本 1）、little-endian/32 位归档指针布局，与玩家共用映射 IO。Windows 保持文件句柄直到映射释放，禁止映射期间写入或删除；其他平台使用匿名映射副本。读取先进行 rkyv bytecheck，再检查区块完整性、调色板、压缩长度、实体归属和箱子方块对应关系。运行时使用稀疏分段网格，网络消息使用协议 9 的 MessagePack。

### 实体数据编解码

`server::entity_data` 参考 [Minecraft Entity format](https://minecraft.wiki/w/Entity_format) 的类型、UUID、Pos、Motion、Rotation、OnGround 和 Health 字段组织，沿用项目自己的 rkyv 格式，不支持 Minecraft NBT 导入。当前生物类型为 `game:zombie`；玩家继续使用独立玩家归档，箱子与地面物品继续保存在区块归档。

实体归档包含 DataVersion、Dimension、批次 UUID、种子、世界 tick，以及按 X/Z 区块列组织的实体列表。全部已生成区块列均显式记录，包括空列表，并与地形区块集合核对；怪物越过区块边界后按当前坐标归档。UUID 与 NPC 网络 ID 均唯一，网络 ID 保持高位命名空间，hecs 句柄不落盘。位置和速度以 f64 存储并恢复为运行时 f32，速度单位为米/秒，角度为弧度；另存生命上限、落地状态和最后攻击 tick，随世界时钟共同恢复，离线时间不推进模拟。当前怪物没有俯仰，Rotation 第二项固定为 0；AI 追踪目标在上线后重新计算。

文件使用 `DNGENT01`、16 字节头和版本 2 数据（兼容读取旧版本 1），沿用 little-endian/32 位归档指针布局和映射 IO。读取先做 rkyv bytecheck，再检查元数据、完整区块集合、坐标归属、类型、UUID/网络 ID、有限数值、生命范围与冷却 tick。实体文件必须与清单和区块归档的批次、种子及世界 tick 一致。空世界不会补怪，缺失文件会报错。

### 玩家数据编解码

`server::player_data` 参考 [Minecraft player.dat](https://minecraft.wiki/w/Player.dat_format) 的字段组织，使用固定版本 `rkyv 0.8.18` + `memmap2 0.9.12`。这是本项目的归档格式，不支持直接导入或导出 Minecraft NBT。

记录包含 `DataVersion`、UUID、名称、Dimension、Pos、Motion、Rotation、OnGround、Health、Inventory 和 SelectedItemSlot。Inventory 仅存非空槽的 `slot/id/count`，保留 0–8 快捷栏和 9–35 背包的空隙，每格最多 64 个；当前物品名称作为项目内物品标识。SelectedItemSlot 为 0–8，不重复保存手持物品。位置用 f64，运行时还原为现有 f32；速度与角度保留项目单位（米/秒、弧度），生命上限保留 100。

`capture/restore` 转换运行时数据；`write_new/read` 读写不可覆盖的玩家归档。文件使用 `DNGPLY01` 标记、长度头、版本 1 数据及固定 little-endian/32 位归档指针布局。Windows 使用文件映射、写入 flush/sync、排他写入和只读映射保护；其他平台读取使用匿名映射副本。读取经过 rkyv bytecheck 和字段范围校验，错误文件不会被重写。恢复后仍需由世界检查碰撞，连接序号和物品栏版本不持久化。

已接入稳定 UUID 登录、同角色重连、改名、周期保存、断线与关闭保存，以及箱子/玩家/地面物品的一致提交。UUID 文件读写在客户端后台网络线程执行；权威存档在服务端处理，Godot 主线程不执行文件读写。测试使用 `target/` 下隔离的世界与身份文件。

| Crate / 目录 | 职责 |
|---|---|
| `protocol/` | 三维消息、区块与角色共享数据、协议版本 |
| `simulation/` (`game-simulation`) | 平台无关的体素网格查询、固定步长运动、Rapier 角色碰撞、占位及视线检测；由服务端和客户端共用 |
| `client-core/voxel` | 三维地图缓存、输入队列、预测与权威状态协调 |
| `client/` | 会话、QUIC 适配器、固定步长调度与显示插值、纯数据网格生成 |
| `godot-extension/` (`game-godot`) | 主线程桥接：输入、会话、快照、网格批量传递；不重复实现玩法 |
| `godot/` | Forward+ 场景、相机、材质、输入、Godot Control UI 与导出设置 |
| `server/voxel` | 权威 3D 世界、地形生成、hecs NPC、战斗、快照与带版本的地图存储 |

旧二维世界、存档读取和客户端状态模块已移除，当前只保留三维实现；旧存档文件仍按上述策略保留，不提供自动迁移。浏览器仍没有窗口宿主与网络适配器；WASM 检查通过不代表浏览器游戏可运行。

### 共享物理

`simulation/src/physics.rs` 使用固定版本 `rapier3d = 0.36.1` 的 `KinematicCharacterController` 和形状扫掠。保留角色盒形尺寸（宽 0.58、高 1.75 米）、移动速度、跳跃和重力；水平移动与垂直移动分别求解，支持贴墙滑动，关闭自动跨台阶与向下吸附。Godot 继续只呈现 Rust 模拟结果，玩家和 NPC 不再经过旧的手写分轴子步碰撞求解器。

每个固定步从当前地形和实体快照构造覆盖移动范围的局部查询；邻接方块合并为盒形碰撞体，减少平地和墙面接缝阻滞。未知格与世界外侧仍是实体障碍，方块增删在下一步立即生效。查询状态不跨步保留，权威纠正及未确认输入重放不会复用旧碰撞缓存；实体按坐标稳定排序，启用 Rapier `enhanced-determinism`，但不据此承诺整套游戏跨平台逐位确定性。

此接入使用运动学角色，不新增刚体堆叠、掉落物弹跳或布娃娃。方块放置和出生恢复的占位检查仍使用同尺寸 AABB；Rapier 接入本身不修改存档与协议消息格式。客户端与服务端应一同更新，旧程序的运动预测不保证与 Rapier 版本一致。

## 验证

执行 `just` 查看带中文说明的全部命令。常用维护命令：

| 命令 | 用途 |
|---|---|
| `just fmt` | 格式化 Rust 代码 |
| `just dev` | 构建并同时启动独立服务端和自动连接的客户端，退出时保存 |
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
just godot-containers
just godot-editing
just godot-commands
just export-debug
just export-release
powershell -File scripts/godot.ps1 demo --capture C:/absolute/path/demo.png
```

`just check` 包含格式和严格 Clippy；大型 Cargo 操作使用 `--jobs 2` 串行运行。WASM 检查仅覆盖协议、核心与共享模拟，需要安装 `wasm32-unknown-unknown` target，不包含原生扩展或桌面 QUIC。

`just godot-containers` 使用 `target/` 下独立世界验证 36 格布局、移动和快速转移、数字键/滚轮、聊天输入隔离、Esc 取消选择，以及箱子存取和保存重开。Rust 测试覆盖堆叠、拆分、合并、交换、容量边界和数量守恒；`client/tests/multiplayer.rs` 另有真实 QUIC 共享箱子、物品栏移动、版本冲突及双客户端方块同步测试。箱子不提供锁定或所有权功能。

`just godot-editing` 使用隔离世界及身份验证左右键、方块收取/消耗、UI 输入隔离、未变化区块网格复用和保存重开。使用 `& $env:GODOT_BIN --path godot --script res://tests/editing.gd` 可在实际图形窗口运行相同检查并保存截图；测试注入输入事件，不代替完整手动键鼠验收。

`just godot-commands` 使用隔离世界验证 `/` 聚焦聊天、指令反馈、飞行升降/加速/悬停、UI 与失焦输入隔离、负坐标传送和区块加载、相机插值重置、错误落点保护及保存重开。使用 `& $env:GODOT_BIN --path godot --script res://tests/commands.gd` 可运行图形检查并保存截图。Rust 回归另覆盖真实 QUIC 双客户端同步、私有反馈、输入重放和飞行碰撞。

Windows 的 just 配方已配置 PowerShell。GPU 截图需要实际图形会话。`godot-smoke` 验证真实场景与扩展，`godot-multiplayer` 启动隔离服务端并验证两个 Godot 桥接实例的真实 QUIC 会话，不能替代两窗口的键鼠操作验收。

导出需安装 Godot 4.7.2 **标准版** Windows x86_64 模板到 `%APPDATA%/Godot/export_templates/4.7.2.stable/`。脚本将模板暂存至 `target/godot-templates/`，从运行时源文件创建独立 `target/godot-export-stage-*/` 项目，并移除其中的编辑器插件和 .NET 配置，再导出不依赖 .NET 的客户端。产物分别位于 Cargo target 目录的 `export-debug/` 和 `export-release/`，发布需一起携带 `Dungeon.exe`、`Dungeon.pck`、`game_godot.dll`，不得混用 debug/release DLL。`just editor` 打开仓库内正式项目。修改 DLL 或运行会导入同一项目的检查前，关闭现有项目编辑器，避免 Windows DLL 占用。

Godot MCP 是可选编辑器工具，已接入本仓库 `godot/`，本地端点为 `http://localhost:17860/mcp`。依赖 Godot 4.7.2 .NET、.NET SDK 8 与 `godot-cli@0.33.0`。`Dungeon.Editor.csproj` 仅编译编辑器插件，游戏仍使用 Rust/GDScript；插件源码和生成的代理技能被 Git 忽略。首次运行先 `just build`，再执行 `powershell -File scripts/godot-mcp.ps1` 安装固定版本插件、构建并启动无窗口编辑器。已打开时先用 `godot-cli close godot --owned-only` 关闭，再更新 DLL 或插件程序集。

操作前通过 MCP 查询 `editor | select projectPath, editedScene`，必须指向本仓库，不能把 `GodotMcpSandbox` 当验收项目。修改后用 `editor | call SaveScene()` 保存。2026-10-27 已通过 MCP 给主场景写入职责说明、保存并回读，主脚本验证无错误。导出排除插件、C#、测试、代理技能和工具配置；运行游戏无需 MCP 服务或编辑器。安装阶段的旧开始菜单快捷方式仍指向沙盒，应使用本仓库脚本启动正式项目。

真实 QUIC 集成测试位于 `client/tests/multiplayer.rs`，覆盖两客户端区块传输、连续移动和跳跃、逐玩家输入确认、聊天、断开释放与旧协议拒绝。验证运行使用独立端口、`target/` 下的存档和证书，不能用真实 `save/` 做破坏性测试。
