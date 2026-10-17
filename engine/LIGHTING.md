# 漫反射全局光照

这是保留 wgpu 30 / WGSL 的第一阶段实现，参考 [kajiya 的 GI 设计](https://github.com/EmbarkStudios/kajiya/blob/main/docs/gi-overview.md)：光栅化 G-buffer，追踪首跳漫反射，在命中点查询随时间反馈的辐照度缓存来近似后续反弹，再做时域去噪与空间重建。本文和源码明确区分已实现的机制与原版 kajiya 的功能。

## 数据与 pass

1. `Renderer::world` 从通用 `Mesh` 建立静态三角形 BVH，上传几何；只有世界网格改变时重建。动态角色网格改变时只重建动态 BVH、上传动态区段。两棵树使用绝对 escape 索引串接，射线也查询屏幕外的几何。引擎不依赖游戏协议或体素类型。
2. G-buffer 保存全分辨率世界坐标（RGBA32Float）、法线（RGBA16Float）、程序材质反照率（RGBA8Unorm）以及深度。使用显式的不可过滤纹理绑定，无需 float32 filtering 或硬件 ray-query 扩展。
3. 每个静态三角形有一项 RGB 辐照度缓存，单位为 irradiance / pi。每帧在三角形上重新选择位置，以两个余弦加权半球样本估计天空或命中表面的出射光；命中表面的太阳光经过遮挡查询，后续反弹读取上帧缓存。最多累计 32 帧权重，并持续更新。缓存使用双缓冲，避免跨线程读写竞争。
4. 每个 2×2 屏幕块追踪一个首跳漫反射样本，代表像素逐帧轮换。该首跳查询当前更新后的缓存。太阳光在有限大小的太阳圆盘上采样，并以独立遮挡射线计算软阴影。
5. 使用上一帧相机和物理像素视口重投影，按世界坐标距离及法线拒绝无效历史。间接光最多保留 32 帧权重，动态网格变化时降到 4 帧；直射阴影最多 4 帧。尺寸、视口、模式和世界变化使相应历史失效。
6. 5×5 半分辨率联合双边滤波按法线、表面平面与空间距离恢复全分辨率光照。合成时乘以材质反照率，执行 Reinhard 色调映射、距离雾及 sRGB 编码，再由宿主独立绘制 HUD。对 sRGB render target 避免重复编码。

CPU 几何保留 static/dynamic 两份，不会每帧重新上传静态世界。GPU 缓冲增长时重新上传静态前缀，动态对象删除后通过实际节点数量排除旧数据。世界变化重建两份缓存并立即丢弃屏幕历史。窗口零尺寸由宿主跳过绘制。高 DPI 大窗口若超过设备 storage-buffer 绑定预算，光照采样步长从 2 自动增至 4、8 等；G-buffer 和 HUD 仍使用原始尺寸。

## 与 kajiya 原版的区别

- 本项目为独立编写的算法适配，未引入 kajiya crate、Vulkan 后端、HLSL 或上游代码片段。设计来源是上述上游文档的 **Indirect diffuse / Irradiance cache / Direct sun shadows** 部分；kajiya 上游采用 MIT OR Apache-2.0 许可。
- 用普通 WGSL compute 遍历软件 BVH，不使用硬件光追；性能不能与 kajiya 公布的 GPU 时间直接比较。
- 当前有限方块场景用按三角形区分的表面缓存，替代上游稀疏 clipmap、查询投票、rank 和 L1 球谐。墙的两侧有不同缓存项，不在它们之间插值。大三角形上的遮挡变化仍会被平均，因此这个缓存适合当前以小方块面为主的场景。
- 采用余弦重要性采样、有限历史平均和双边滤波，**尚未移植 ReSTIR reservoir 的时空重采样、样本验证或上游降噪器**。不能称为完整 kajiya 或完整 ReSTIR GI。
- 当前只覆盖漫反射与太阳阴影；镜面反射、GGX 材质、动态天空、发光材质、TAA/超分辨率和 DLSS 不在这一阶段。
- 动态网格参与阴影、首跳和缓存更新的射线命中，但不拥有长期表面缓存；从动态表面继续的高阶间接反弹尚未实现。自身体模型仍遵循现有第一人称隐藏规则。
- 间接光使用网格的线性基础反照率；程序化砖缝等纹理只在主视图中计算。叶子、水面仍按原有不透明几何处理。
- 历史滤波和缓存需要收敛；快速运动、刚显露的区域与极细几何可能出现短暂噪声、拖尾或半分辨率细节损失。没有承诺所有 GPU 均达到实时帧率。

## 使用与验证

默认开启 GI；Tab 打开侧栏可切换 Global illumination / Direct light only / Indirect light only / Original lighting。UI 仅收集切换命令，布局结束后应用一次。原始模式跳过 GI compute，用于视觉对照；其色调映射与新模式不同，不适合用像素差直接衡量 GI 能量。

```sh
cargo run -p client -- --demo --lighting gi --capture target/lighting-gi.png
cargo run -p client -- --demo --lighting direct --capture target/lighting-direct.png
cargo run -p client -- --demo --lighting indirect --capture target/lighting-indirect.png
cargo run -p client -- --demo --lighting legacy --capture target/lighting-legacy.png
cargo test -p game-engine --test lighting_gpu --jobs 2 -- --ignored --nocapture
```

截图默认等待世界就绪后的 64 帧，可用 `--capture-frames 1..4096` 调整；世界网格重建时重新计数，整体超时 120 秒。采样序列按帧确定，可重复比较同一静态场景。截图不是“第一帧已收敛”的保证。

普通单元测试检查 BVH 的包围盒、跳转和动态偏移。显式 GPU 测试使用独立离屏网格，验证间接照明、红/蓝墙的颜色反弹、密闭房间不漏光、场景替换不残留历史、奇数尺寸重建，以及屏幕外动态遮挡物添加/删除。该测试默认忽略，需显式执行；没有图形适配器会报错，不会静默跳过后宣称通过。

`just web-check` 仅验证可移植 crate 的 WASM 编译；当前仍没有浏览器宿主。GPU 图像验证与 Rust/Clippy 检查必须分别执行。
