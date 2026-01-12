use anyhow::{Context, Result, bail};
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::time::Duration;

pub struct AudioPlayer {
    _stream: OutputStream,
    _stream_handle: OutputStreamHandle,
    sink: Sink,
    volume: Arc<AtomicU8>,
    is_playing: Arc<AtomicBool>,
}

impl AudioPlayer {
    pub fn new() -> Result<Self> {
        let (stream, stream_handle) =
            OutputStream::try_default().with_context(|| "Failed to open audio output device")?;

        let sink = Sink::try_new(&stream_handle).with_context(|| "Failed to create audio sink")?;

        let volume = Arc::new(AtomicU8::new(80));
        let is_playing = Arc::new(AtomicBool::new(false));

        sink.set_volume(0.8);

        Ok(Self {
            _stream: stream,
            _stream_handle: stream_handle,
            sink,
            volume,
            is_playing,
        })
    }

    pub fn play_file(&self, path: &Path) -> Result<()> {
        if !path.exists() {
            bail!("Audio file not found: {}", path.display());
        }

        let file = File::open(path)
            .with_context(|| format!("Failed to open audio file: {}", path.display()))?;

        let reader = BufReader::new(file);
        let source = Decoder::new(reader)
            .with_context(|| format!("Failed to decode audio file: {}", path.display()))?;

        self.sink.clear();
        self.sink.append(source);
        self.sink.play();
        self.is_playing.store(true, Ordering::SeqCst);

        Ok(())
    }

    pub fn pause(&self) {
        self.sink.pause();
        self.is_playing.store(false, Ordering::SeqCst);
    }

    pub fn resume(&self) {
        self.sink.play();
        if !self.sink.empty() {
            self.is_playing.store(true, Ordering::SeqCst);
        }
    }

    pub fn stop(&self) {
        self.sink.stop();
        self.is_playing.store(false, Ordering::SeqCst);
    }

    pub fn set_volume(&self, volume: u8) {
        let vol = volume.min(100);
        self.volume.store(vol, Ordering::SeqCst);
        self.sink.set_volume(vol as f32 / 100.0);
    }

    #[allow(dead_code)]
    pub fn get_volume(&self) -> u8 {
        self.volume.load(Ordering::SeqCst)
    }

    pub fn seek(&self, position: Duration) -> bool {
        self.sink.try_seek(position).is_ok()
    }

    pub fn get_position(&self) -> Duration {
        self.sink.get_pos()
    }

    #[allow(dead_code)]
    pub fn is_playing(&self) -> bool {
        self.is_playing.load(Ordering::SeqCst) && !self.sink.is_paused()
    }

    #[allow(dead_code)]
    pub fn is_paused(&self) -> bool {
        self.sink.is_paused()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.sink.empty()
    }

    pub fn is_finished(&self) -> bool {
        self.sink.empty() && !self.sink.is_paused()
    }

    #[allow(dead_code)]
    pub fn sleep_until_end(&self) {
        self.sink.sleep_until_end();
    }

    #[allow(dead_code)]
    pub fn wait_for_playback(&self, timeout: Duration) -> bool {
        let start = std::time::Instant::now();
        while !self.is_finished() {
            if start.elapsed() > timeout {
                return false;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        true
    }
}

impl Drop for AudioPlayer {
    fn drop(&mut self) {
        self.sink.stop();
    }
}
