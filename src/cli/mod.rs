use clap::{Parser, Subcommand};

use crate::models::RepeatMode;

mod commands;
pub use commands::*;

#[derive(Parser)]
#[command(name = "mixyt")]
#[command(about = "A CLI tool for saving, managing, and playing YouTube audio")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Add a track to the library from a YouTube URL
    Add {
        /// YouTube URL
        url: String,
        /// Optional alias for quick reference
        #[arg(short, long)]
        alias: Option<String>,
    },

    /// Remove a track from the library
    Remove {
        /// Track name, alias, or search query
        query: String,
    },

    /// Play a track
    Play {
        /// Track name, alias, or search query
        query: String,
    },

    /// Pause playback
    Pause,

    /// Resume playback
    Resume,

    /// Stop playback
    Stop,

    /// Skip to the next track
    Next,

    /// Go to the previous track
    #[command(name = "prev")]
    Previous,

    /// Seek to a position (e.g., "1:30" or "90")
    Seek {
        /// Position in seconds or MM:SS format
        position: String,
    },

    /// Set or show volume
    Volume {
        /// Volume level (0-100)
        level: Option<u8>,
    },

    /// List tracks in the library
    List {
        /// Filter by playlist name
        #[arg(short, long)]
        playlist: Option<String>,
    },

    /// Search the library
    Search {
        /// Search query
        query: String,
    },

    /// Playlist management
    Playlist {
        #[command(subcommand)]
        command: PlaylistCommands,
    },

    /// Queue management
    Queue {
        #[command(subcommand)]
        command: QueueCommands,
    },

    /// Toggle or set shuffle mode
    Shuffle {
        /// on or off
        mode: Option<String>,
    },

    /// Set repeat mode
    Repeat {
        /// off, one, or all
        mode: Option<RepeatMode>,
    },

    /// Show current playback status
    Status,

    /// Daemon management
    Daemon {
        #[command(subcommand)]
        command: DaemonCommands,
    },

    /// Export library to JSON
    Export {
        /// Output file path
        #[arg(short, long)]
        file: Option<String>,
    },

    /// Import library from JSON
    Import {
        /// Input file path
        file: String,
    },

    /// Check track availability
    Check,

    /// Launch interactive TUI
    #[command(name = "tui")]
    Tui,
}

#[derive(Subcommand)]
pub enum PlaylistCommands {
    /// Create a new playlist
    Create {
        /// Playlist name
        name: String,
    },
    /// Add a track to a playlist
    Add {
        /// Playlist name
        playlist: String,
        /// Track query
        query: String,
    },
    /// Remove a track from a playlist
    Remove {
        /// Playlist name
        playlist: String,
        /// Track query
        query: String,
    },
    /// Delete a playlist
    Delete {
        /// Playlist name
        name: String,
    },
    /// List all playlists
    List,
    /// Show tracks in a playlist
    Show {
        /// Playlist name
        name: String,
    },
    /// Play a playlist
    Play {
        /// Playlist name
        name: String,
        /// Start with shuffle
        #[arg(short, long)]
        shuffle: bool,
    },
}

#[derive(Subcommand)]
pub enum QueueCommands {
    /// Add a track to the queue
    Add {
        /// Track query
        query: String,
    },
    /// Show the current queue
    List,
    /// Clear the queue
    Clear,
}

#[derive(Subcommand)]
pub enum DaemonCommands {
    /// Start the daemon
    Start,
    /// Stop the daemon
    Stop,
    /// Show daemon status
    Status,
    /// Run daemon in foreground (internal use)
    Run,
}
