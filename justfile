set windows-shell := ["powershell.exe", "-NoLogo", "-NoProfile", "-Command"]

default:
    @just --list

server:
    cargo run -p server --jobs 2

client:
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/godot.ps1 run

demo:
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/godot.ps1 demo

build:
    cargo build --workspace --jobs 2
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/godot.ps1 build

check:
    cargo fmt --all -- --check
    cargo clippy --workspace --all-targets --jobs 2 -- -D warnings

test:
    cargo test --workspace --jobs 2

web-check:
    cargo check -p protocol -p game-client-core -p game-simulation --target wasm32-unknown-unknown --jobs 2

editor:
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/godot.ps1 editor

godot-smoke:
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/godot.ps1 smoke

godot-multiplayer:
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/godot.ps1 multiplayer

export-debug:
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/godot.ps1 export-debug

export-release:
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/godot.ps1 export-release
