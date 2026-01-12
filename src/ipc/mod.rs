use anyhow::{Context, Result};
use interprocess::TryClone;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

use crate::models::{PlaybackState, RepeatMode, Track};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DaemonCommand {
    Play {
        track: Track,
    },
    PlayQueue {
        tracks: Vec<Track>,
        start_index: usize,
    },
    Pause,
    Resume,
    Stop,
    Next,
    Previous,
    Seek {
        position: u64,
    },
    SetVolume {
        volume: u8,
    },
    SetShuffle {
        enabled: bool,
    },
    SetRepeat {
        mode: RepeatMode,
    },
    QueueAdd {
        track: Track,
    },
    QueueClear,
    GetStatus,
    Shutdown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DaemonResponse {
    Ok,
    Status(PlaybackState),
    Error(String),
}

pub struct DaemonClient {
    socket_path: std::path::PathBuf,
}

impl DaemonClient {
    pub fn new(socket_path: impl AsRef<Path>) -> Self {
        Self {
            socket_path: socket_path.as_ref().to_path_buf(),
        }
    }

    pub fn is_daemon_running(&self) -> bool {
        self.socket_path.exists() && self.send_command(DaemonCommand::GetStatus).is_ok()
    }

    pub fn send_command(&self, command: DaemonCommand) -> Result<DaemonResponse> {
        use interprocess::local_socket::GenericFilePath;
        use interprocess::local_socket::prelude::*;

        let path = self.socket_path.as_os_str();
        let name = path
            .to_fs_name::<GenericFilePath>()
            .with_context(|| "Invalid socket path")?;

        let conn = interprocess::local_socket::Stream::connect(name).with_context(|| {
            format!(
                "Failed to connect to daemon at {}",
                self.socket_path.display()
            )
        })?;

        let mut writer = conn;
        let mut reader = BufReader::new(writer.try_clone()?);

        // Send command
        let msg = serde_json::to_string(&command)?;
        writeln!(writer, "{msg}")?;
        writer.flush()?;

        // Read response
        let mut response_line = String::new();
        reader.read_line(&mut response_line)?;

        let response: DaemonResponse = serde_json::from_str(&response_line)
            .with_context(|| "Failed to parse daemon response")?;

        Ok(response)
    }

    pub fn play(&self, track: Track) -> Result<DaemonResponse> {
        self.send_command(DaemonCommand::Play { track })
    }

    pub fn play_queue(&self, tracks: Vec<Track>, start_index: usize) -> Result<DaemonResponse> {
        self.send_command(DaemonCommand::PlayQueue {
            tracks,
            start_index,
        })
    }

    pub fn pause(&self) -> Result<DaemonResponse> {
        self.send_command(DaemonCommand::Pause)
    }

    pub fn resume(&self) -> Result<DaemonResponse> {
        self.send_command(DaemonCommand::Resume)
    }

    pub fn stop(&self) -> Result<DaemonResponse> {
        self.send_command(DaemonCommand::Stop)
    }

    pub fn next(&self) -> Result<DaemonResponse> {
        self.send_command(DaemonCommand::Next)
    }

    pub fn previous(&self) -> Result<DaemonResponse> {
        self.send_command(DaemonCommand::Previous)
    }

    pub fn seek(&self, position: u64) -> Result<DaemonResponse> {
        self.send_command(DaemonCommand::Seek { position })
    }

    pub fn set_volume(&self, volume: u8) -> Result<DaemonResponse> {
        self.send_command(DaemonCommand::SetVolume { volume })
    }

    pub fn set_shuffle(&self, enabled: bool) -> Result<DaemonResponse> {
        self.send_command(DaemonCommand::SetShuffle { enabled })
    }

    pub fn set_repeat(&self, mode: RepeatMode) -> Result<DaemonResponse> {
        self.send_command(DaemonCommand::SetRepeat { mode })
    }

    pub fn queue_add(&self, track: Track) -> Result<DaemonResponse> {
        self.send_command(DaemonCommand::QueueAdd { track })
    }

    pub fn queue_clear(&self) -> Result<DaemonResponse> {
        self.send_command(DaemonCommand::QueueClear)
    }

    pub fn get_status(&self) -> Result<PlaybackState> {
        match self.send_command(DaemonCommand::GetStatus)? {
            DaemonResponse::Status(state) => Ok(state),
            DaemonResponse::Error(e) => anyhow::bail!("{e}"),
            _ => anyhow::bail!("Unexpected response"),
        }
    }

    pub fn shutdown(&self) -> Result<DaemonResponse> {
        self.send_command(DaemonCommand::Shutdown)
    }
}
