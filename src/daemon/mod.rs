use anyhow::{Context, Result};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use tracing::{error, info};

use crate::audio::AudioPlayer;
use crate::config::Config;
use crate::ipc::{DaemonCommand, DaemonResponse};
use crate::models::{PlaybackState, RepeatMode, Track};

// Media key support is platform-specific and requires additional setup
// Placeholder modules exist but are not spawned by default
#[cfg(target_os = "macos")]
mod mediakeys;
#[cfg(target_os = "linux")]
mod mediakeys;

// Internal commands for the audio thread
enum AudioCommand {
    Play(Track),
    Pause,
    Resume,
    Stop,
    SetVolume(u8),
    CheckFinished(Sender<bool>),
}

pub struct Daemon {
    config: Config,
}

impl Daemon {
    pub fn new(config: Config) -> Result<Self> {
        Ok(Self { config })
    }

    pub fn run(&self) -> Result<()> {
        use interprocess::local_socket::prelude::*;
        use interprocess::local_socket::{GenericFilePath, ListenerOptions};

        let socket_path = self.config.socket_path();

        // Remove stale socket
        if socket_path.exists() {
            fs::remove_file(&socket_path)?;
        }

        // Write PID file
        let pid_path = self.config.pid_path();
        fs::write(&pid_path, std::process::id().to_string())?;

        // Create listener
        let name = socket_path.as_os_str().to_fs_name::<GenericFilePath>()?;
        let listener = ListenerOptions::new()
            .name(name)
            .create_sync()
            .with_context(|| "Failed to create socket listener")?;

        info!("Daemon started, listening on {}", socket_path.display());

        // Shared state
        let state = Arc::new(Mutex::new(PlaybackState::new()));
        state.lock().unwrap().volume = self.config.playback.default_volume;

        let running = Arc::new(AtomicBool::new(true));

        // Create channel for audio commands
        let (audio_tx, audio_rx): (Sender<AudioCommand>, Receiver<AudioCommand>) = mpsc::channel();

        // Spawn audio thread - AudioPlayer stays on this single thread
        let audio_running = Arc::clone(&running);
        let audio_state = Arc::clone(&state);
        let default_volume = self.config.playback.default_volume;
        thread::spawn(move || {
            run_audio_thread(audio_rx, audio_state, audio_running, default_volume);
        });

        // Spawn playback monitor thread
        let monitor_state = Arc::clone(&state);
        let monitor_running = Arc::clone(&running);
        let monitor_audio_tx = audio_tx.clone();
        thread::spawn(move || {
            playback_monitor(monitor_state, monitor_running, monitor_audio_tx);
        });

        // Accept connections on main thread
        while running.load(Ordering::SeqCst) {
            match listener.accept() {
                Ok(conn) => {
                    let response = handle_connection(conn, &state, &running, &audio_tx);

                    if let Err(e) = response {
                        error!("Connection error: {e}");
                    }
                }
                Err(e) => {
                    if running.load(Ordering::SeqCst) {
                        error!("Accept error: {e}");
                    }
                }
            }
        }

        // Cleanup
        let _ = fs::remove_file(&socket_path);
        let _ = fs::remove_file(&pid_path);

        info!("Daemon stopped");
        Ok(())
    }

    pub fn start_detached(config: &Config) -> Result<()> {
        use std::process::Command;

        let socket_path = config.socket_path();
        if socket_path.exists() {
            let client = crate::ipc::DaemonClient::new(&socket_path);
            if client.is_daemon_running() {
                anyhow::bail!("Daemon is already running");
            }
            fs::remove_file(&socket_path)?;
        }

        let exe = std::env::current_exe()?;

        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;

            Command::new(&exe)
                .arg("daemon")
                .arg("run")
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .process_group(0)
                .spawn()
                .with_context(|| "Failed to start daemon")?;
        }

        for _ in 0..50 {
            if socket_path.exists() {
                return Ok(());
            }
            thread::sleep(std::time::Duration::from_millis(100));
        }

        anyhow::bail!("Daemon failed to start")
    }

    pub fn stop(config: &Config) -> Result<()> {
        let client = crate::ipc::DaemonClient::new(config.socket_path());
        if client.is_daemon_running() {
            client.shutdown()?;
            for _ in 0..50 {
                if !config.socket_path().exists() {
                    return Ok(());
                }
                thread::sleep(std::time::Duration::from_millis(100));
            }
        }
        Ok(())
    }

    pub fn is_running(config: &Config) -> bool {
        let client = crate::ipc::DaemonClient::new(config.socket_path());
        client.is_daemon_running()
    }
}

fn run_audio_thread(
    rx: Receiver<AudioCommand>,
    state: Arc<Mutex<PlaybackState>>,
    running: Arc<AtomicBool>,
    default_volume: u8,
) {
    let player = match AudioPlayer::new() {
        Ok(p) => {
            p.set_volume(default_volume);
            Some(p)
        }
        Err(e) => {
            error!("Failed to initialize audio player: {e}");
            None
        }
    };

    while running.load(Ordering::SeqCst) {
        match rx.recv_timeout(std::time::Duration::from_millis(100)) {
            Ok(cmd) => {
                if let Some(ref p) = player {
                    match cmd {
                        AudioCommand::Play(track) => {
                            let path = std::path::Path::new(&track.file_path);
                            if let Err(e) = p.play_file(path) {
                                error!("Failed to play: {e}");
                                state.lock().unwrap().is_playing = false;
                            } else {
                                let mut s = state.lock().unwrap();
                                s.current_track = Some(track);
                                s.is_playing = true;
                                s.position = 0;
                            }
                        }
                        AudioCommand::Pause => {
                            p.pause();
                            state.lock().unwrap().is_playing = false;
                        }
                        AudioCommand::Resume => {
                            p.resume();
                            state.lock().unwrap().is_playing = true;
                        }
                        AudioCommand::Stop => {
                            p.stop();
                            let mut s = state.lock().unwrap();
                            s.is_playing = false;
                            s.current_track = None;
                            s.position = 0;
                        }
                        AudioCommand::SetVolume(vol) => {
                            p.set_volume(vol);
                            state.lock().unwrap().volume = vol;
                        }
                        AudioCommand::CheckFinished(response_tx) => {
                            let _ = response_tx.send(p.is_finished());
                        }
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
}

fn playback_monitor(
    state: Arc<Mutex<PlaybackState>>,
    running: Arc<AtomicBool>,
    audio_tx: Sender<AudioCommand>,
) {
    while running.load(Ordering::SeqCst) {
        thread::sleep(std::time::Duration::from_millis(500));

        let should_check = {
            let s = state.lock().unwrap();
            s.is_playing && s.current_track.is_some()
        };

        if !should_check {
            continue;
        }

        // Check if audio finished
        let (tx, rx) = mpsc::channel();
        let sent = audio_tx.send(AudioCommand::CheckFinished(tx)).is_ok();
        let finished = sent
            && rx
                .recv_timeout(std::time::Duration::from_millis(100))
                .unwrap_or(false);

        if finished {
            let next_track = {
                let mut s = state.lock().unwrap();
                let repeat = s.repeat;

                if repeat == RepeatMode::One {
                    s.current_track.clone()
                } else if !s.queue.is_empty() {
                    let next_idx = if s.shuffle {
                        use std::collections::hash_map::RandomState;
                        use std::hash::{BuildHasher, Hasher};
                        let random = RandomState::new().build_hasher().finish() as usize;
                        random % s.queue.len()
                    } else {
                        (s.queue_index + 1) % s.queue.len()
                    };

                    if !s.shuffle && next_idx == 0 && repeat == RepeatMode::Off {
                        s.is_playing = false;
                        s.current_track = None;
                        None
                    } else {
                        s.queue_index = next_idx;
                        Some(s.queue[next_idx].clone())
                    }
                } else {
                    s.is_playing = false;
                    s.current_track = None;
                    None
                }
            };

            if let Some(track) = next_track {
                let _ = audio_tx.send(AudioCommand::Play(track));
            }
        }
    }
}

fn handle_connection(
    conn: interprocess::local_socket::Stream,
    state: &Arc<Mutex<PlaybackState>>,
    running: &Arc<AtomicBool>,
    audio_tx: &Sender<AudioCommand>,
) -> Result<()> {
    let mut reader = BufReader::new(&conn);
    let mut writer = &conn;

    let mut line = String::new();
    reader.read_line(&mut line)?;

    let command: DaemonCommand = serde_json::from_str(&line)?;
    let response = handle_command(command, state, running, audio_tx);

    let response_json = serde_json::to_string(&response)?;
    writeln!(writer, "{response_json}")?;
    writer.flush()?;

    Ok(())
}

fn handle_command(
    command: DaemonCommand,
    state: &Arc<Mutex<PlaybackState>>,
    running: &Arc<AtomicBool>,
    audio_tx: &Sender<AudioCommand>,
) -> DaemonResponse {
    match command {
        DaemonCommand::Play { track } => {
            if audio_tx.send(AudioCommand::Play(track)).is_ok() {
                DaemonResponse::Ok
            } else {
                DaemonResponse::Error("Audio thread not running".to_string())
            }
        }
        DaemonCommand::PlayQueue {
            tracks,
            start_index,
        } => {
            if tracks.is_empty() {
                return DaemonResponse::Error("Queue is empty".to_string());
            }

            let idx = start_index.min(tracks.len() - 1);
            let track = tracks[idx].clone();

            {
                let mut s = state.lock().unwrap();
                s.queue = tracks;
                s.queue_index = idx;
            }

            if audio_tx.send(AudioCommand::Play(track)).is_ok() {
                DaemonResponse::Ok
            } else {
                DaemonResponse::Error("Audio thread not running".to_string())
            }
        }
        DaemonCommand::Pause => {
            let _ = audio_tx.send(AudioCommand::Pause);
            DaemonResponse::Ok
        }
        DaemonCommand::Resume => {
            let _ = audio_tx.send(AudioCommand::Resume);
            DaemonResponse::Ok
        }
        DaemonCommand::Stop => {
            let _ = audio_tx.send(AudioCommand::Stop);
            DaemonResponse::Ok
        }
        DaemonCommand::Next => {
            let next_track = {
                let mut s = state.lock().unwrap();
                if s.queue.is_empty() {
                    return DaemonResponse::Error("Queue is empty".to_string());
                }

                let next_idx = if s.shuffle {
                    use std::collections::hash_map::RandomState;
                    use std::hash::{BuildHasher, Hasher};
                    let random = RandomState::new().build_hasher().finish() as usize;
                    random % s.queue.len()
                } else {
                    (s.queue_index + 1) % s.queue.len()
                };

                if !s.shuffle && next_idx == 0 && s.repeat == RepeatMode::Off {
                    s.is_playing = false;
                    s.current_track = None;
                    return DaemonResponse::Ok;
                }

                s.queue_index = next_idx;
                s.queue[next_idx].clone()
            };

            if audio_tx.send(AudioCommand::Play(next_track)).is_ok() {
                DaemonResponse::Ok
            } else {
                DaemonResponse::Error("Audio thread not running".to_string())
            }
        }
        DaemonCommand::Previous => {
            let prev_track = {
                let mut s = state.lock().unwrap();
                if s.queue.is_empty() {
                    return DaemonResponse::Error("Queue is empty".to_string());
                }

                let prev_idx = if s.queue_index == 0 {
                    s.queue.len() - 1
                } else {
                    s.queue_index - 1
                };

                s.queue_index = prev_idx;
                s.queue[prev_idx].clone()
            };

            if audio_tx.send(AudioCommand::Play(prev_track)).is_ok() {
                DaemonResponse::Ok
            } else {
                DaemonResponse::Error("Audio thread not running".to_string())
            }
        }
        DaemonCommand::Seek { position } => {
            state.lock().unwrap().position = position;
            DaemonResponse::Ok
        }
        DaemonCommand::SetVolume { volume } => {
            let _ = audio_tx.send(AudioCommand::SetVolume(volume));
            DaemonResponse::Ok
        }
        DaemonCommand::SetShuffle { enabled } => {
            state.lock().unwrap().shuffle = enabled;
            DaemonResponse::Ok
        }
        DaemonCommand::SetRepeat { mode } => {
            state.lock().unwrap().repeat = mode;
            DaemonResponse::Ok
        }
        DaemonCommand::QueueAdd { track } => {
            state.lock().unwrap().queue.push(track);
            DaemonResponse::Ok
        }
        DaemonCommand::QueueClear => {
            let mut s = state.lock().unwrap();
            s.queue.clear();
            s.queue_index = 0;
            DaemonResponse::Ok
        }
        DaemonCommand::GetStatus => {
            let s = state.lock().unwrap().clone();
            DaemonResponse::Status(s)
        }
        DaemonCommand::Shutdown => {
            running.store(false, Ordering::SeqCst);
            let _ = audio_tx.send(AudioCommand::Stop);
            DaemonResponse::Ok
        }
    }
}
