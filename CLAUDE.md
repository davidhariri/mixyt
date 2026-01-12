# CLAUDE.md

## Project Overview

mixyt is a macOS-only CLI tool for saving and playing YouTube audio from the terminal. It downloads audio via yt-dlp and plays it through a background daemon.

## Architecture

- **Daemon** (`src/daemon/mod.rs`): Background process that handles audio playback. Communicates via Unix socket IPC.
- **TUI** (`src/tui/mod.rs`): Terminal UI built with ratatui. Connects to daemon as a client.
- **Audio** (`src/audio/mod.rs`): Wrapper around rodio for playback, seeking, volume control.
- **CLI** (`src/cli/`): clap-based commands. Running `mixyt` with no args opens the TUI.
- **DB** (`src/db/mod.rs`): SQLite database storing tracks (title, URL, file path, duration).
- **IPC** (`src/ipc/mod.rs`): Client for communicating with the daemon over Unix socket.
- **Downloader** (`src/downloader/mod.rs`): Wraps yt-dlp to fetch video info and download audio.

## Development

```bash
# Build
cargo build

# Run TUI (starts daemon automatically if not running)
cargo run

# Run with specific command
cargo run -- add "https://youtube.com/watch?v=..."
cargo run -- help

# Tests
cargo test

# Lint
cargo clippy
cargo fmt --check
```

## External Dependencies

Users must have installed: `brew install yt-dlp ffmpeg`

## Key Patterns

- The daemon runs in background and owns the audio player. TUI/CLI are clients.
- Tracks are stored in `~/.mixyt/tracks/` as audio files, metadata in SQLite at `~/.mixyt/mixyt.db`.
- IPC uses JSON-serialized commands/responses over Unix socket at `~/.mixyt/daemon.sock`.

## Known Issues

- Media key support (souvlaki) is implemented but doesn't work reliably on macOS.

## Release Process

1. Update version in `Cargo.toml`
2. Commit, tag with `vX.Y.Z`, push both
3. GitHub Actions builds and creates release with macOS binaries
