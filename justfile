default:
    @just --list

server:
    cargo run -p server

client:
    cargo run -p client -- --connect

demo:
    cargo run -p client -- --demo

build:
    cargo build --workspace --jobs 2

check:
    cargo fmt --all -- --check
    cargo clippy --workspace --all-targets --jobs 2 -- -D warnings

test:
    cargo test --workspace --jobs 2

web-check:
    cargo check -p game-client-core -p game-simulation -p game-engine --target wasm32-unknown-unknown --jobs 2
