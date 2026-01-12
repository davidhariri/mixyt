# mixyt

A CLI tool for saving, managing, and playing YouTube audio from the terminal.

**macOS only** (Apple Silicon and Intel)

## Features

- **Save audio** from YouTube URLs to a local library
- **Rename tracks** with custom aliases
- **Fuzzy search** to find tracks quickly
- **Background playback** via daemon - keeps playing while you work
- **Media keys** - control playback from anywhere with system media keys
- **Interactive TUI** for visual browsing and adding tracks
- **Export/import** your library as JSON

## Installation

```bash
curl -fsSL https://raw.githubusercontent.com/davidhariri/mixyt/main/install.sh | sh
```

You'll also need `yt-dlp` and `ffmpeg`:

```bash
brew install yt-dlp ffmpeg
```

### Alternative: From source

```bash
cargo install --git https://github.com/davidhariri/mixyt
```

## Quick Start

```bash
# Add a track
mixyt add "https://www.youtube.com/watch?v=dQw4w9WgXcQ"

# Open the player (TUI is the default)
mixyt

# Or play directly from command line
mixyt play lofi
```

## Usage

### Library Management

```bash
mixyt add <url>                  # Add a track
mixyt add <url> --alias chill    # Add with a short alias
mixyt remove <query>             # Remove a track
mixyt list                       # List all tracks
mixyt search <query>             # Fuzzy search
mixyt check                      # Verify track availability
```

### Playback

```bash
mixyt play <query>    # Play a track
mixyt pause           # Pause playback
mixyt resume          # Resume playback
mixyt stop            # Stop playback
mixyt seek 1:30       # Seek to position (MM:SS or seconds)
mixyt volume 80       # Set volume (0-100)
mixyt status          # Show current status
```

### Daemon

The daemon runs in the background to handle playback. It starts automatically when you play a track.

```bash
mixyt daemon start    # Start manually
mixyt daemon stop     # Stop the daemon
mixyt daemon status   # Check if running
```

### Export & Import

```bash
mixyt export --file backup.json   # Export library
mixyt import backup.json          # Import library
```

### Interactive TUI

```bash
mixyt      # TUI is the default
mixyt tui  # Or explicitly
```

**Keyboard shortcuts:**
- `q` - Quit
- `j/k` or arrows - Navigate
- `Enter` or `Space` - Play selected track (or pause/resume if playing)
- `+/-` - Volume up/down
- `/` - Search
- `a` - Add track (paste YouTube URL)
- `e` - Edit (rename) selected track

**Media keys:**
Use your keyboard's play/pause media keys to control playback from any app.

## Configuration

Config file: `~/.config/mixyt/config.toml`

```toml
[storage]
path = "~/.mixyt"

[audio]
format = "mp3"
quality = "best"

[daemon]
auto_start = true

[playback]
default_volume = 80
```

## Data Storage

```
~/.mixyt/
├── mixyt.db      # SQLite database
├── audio/        # Downloaded audio files
└── mixyt.sock    # Daemon socket (when running)
```

## Building from Source

```bash
git clone https://github.com/davidhariri/mixyt
cd mixyt
cargo build --release
./target/release/mixyt --help
```

## License

MIT License - see [LICENSE](LICENSE)
