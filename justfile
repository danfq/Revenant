# Show available commands.
default:
    @just --list

# Install the frontend dependencies from the lockfile.
install:
    cd gui && bun install --frozen-lockfile

# Start the desktop backend and Astro frontend (mode: dev or release).
run mode="dev":
    #!/usr/bin/env sh
    set -eu
    cd gui
    case "{{ mode }}" in
        dev) exec bun run tauri dev ;;
        release) exec bun run tauri dev --release ;;
        *) echo 'Mode must be dev or release' >&2; exit 2 ;;
    esac

# Start development mode with hot reload.
dev:
    @just run dev

# Start the desktop app with a release backend and the Astro dev server.
prod:
    @just run release

# Build the release CLI and desktop app, including the frontend and bundles.
build:
    cargo build --locked --release -p revenant
    cd gui && bun run tauri build

# Remove Rust artifacts and project-local frontend build caches.
clean:
    cargo clean
    rm -rf gui/dist gui/node_modules/.vite gui/node_modules/.vite-temp
    find gui -maxdepth 1 -name '*.tsbuildinfo' -delete

# Run Rust tests, frontend type checking, and any frontend tests.
test:
    cargo test --locked --workspace
    cd gui && bun run check
    cd gui && bun run test

# Format Rust and all Prettier-supported files throughout the project.
format:
    cargo fmt --all
    bun ./gui/node_modules/prettier/bin/prettier.cjs --write .

# Check formatting without modifying files.
format-check:
    cargo fmt --all --check
    bun ./gui/node_modules/prettier/bin/prettier.cjs --check .
