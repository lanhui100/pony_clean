set shell := ["pwsh", "-c"]
set windows-shell := ["pwsh", "-c"]

default:
    @just --list

build:
    cargo build

run:
    cargo run

test:
    cargo test

lint:
    cargo clippy -- -D clippy::all -D clippy::pedantic -A clippy::allow_attributes_without_reason

fmt:
    cargo fmt

fmt-check:
    cargo fmt --check

clean:
    cargo clean

doc:
    cargo doc --no-deps

ci: fmt-check build lint test

release:
    cargo build --release
