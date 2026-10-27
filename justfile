set windows-shell := ["powershell.exe", "-NoLogo", "-NoProfile", "-Command"]

# 列出所有常用命令
default:
    @just --list

# 启动独立 Rust 服务端
server:
    cargo run -p server --jobs 2

# 构建扩展并启动 Godot 客户端
client:
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/godot.ps1 run

# 同时启动独立服务端与客户端，客户端退出后保存并关闭服务端
dev:
    cargo +nightly -Zscript scripts/dev.rs

# 启动离线漫游
demo:
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/godot.ps1 demo

# 构建整个工作区并安装 Godot 扩展 DLL
build:
    cargo build --workspace --jobs 2
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/godot.ps1 build

# 格式化所有 Rust 代码
fmt:
    cargo fmt --all

# 检查 Rust 格式，不修改文件
fmt-check:
    cargo fmt --all -- --check

# 严格检查全部 Rust target，警告视为错误
clippy:
    cargo clippy --workspace --all-targets --jobs 2 -- -D warnings

# 依次执行格式检查和严格 Clippy
check: fmt-check clippy

# 运行工作区全部 Rust 测试
test:
    cargo test --workspace --jobs 2

# 单独运行真实 QUIC 多客户端集成测试
test-multiplayer:
    cargo test -p client --test multiplayer --jobs 2

# 生成工作区 API 文档（不生成依赖文档）
doc:
    cargo doc --workspace --no-deps --jobs 2

# 清理 Cargo 构建缓存；不清理 Godot 缓存或已安装的 DLL
clean:
    cargo clean

# 检查纯 Rust 核心的 WASM 可移植性
web-check:
    cargo check -p protocol -p game-client-core -p game-simulation --target wasm32-unknown-unknown --jobs 2

# 构建扩展并打开正式项目编辑器
editor:
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/godot.ps1 editor

# 检查 Godot 导入、扩展加载和离线场景
godot-smoke:
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/godot.ps1 smoke

# 使用隔离服务端检查两个 Godot 桥接实例联机
godot-multiplayer:
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/godot.ps1 multiplayer

# 使用隔离世界验证箱子 UI、存取和保存重开
godot-containers:
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/godot.ps1 containers

# 导出 Windows 调试客户端
export-debug:
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/godot.ps1 export-debug

# 导出 Windows 发布客户端
export-release:
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/godot.ps1 export-release
