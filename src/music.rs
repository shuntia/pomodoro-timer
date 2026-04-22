use lofty::file::AudioFile as _;
use rodio::{Decoder, OutputStream, Sink};
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub fn is_video_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| VIDEO_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

const VIDEO_EXTENSIONS: &[&str] = &[
    "mp4", "mkv", "webm", "avi", "mov", "m4v", "wmv", "flv", "ts", "m2ts",
];

pub struct MusicPlayer {
    files: Vec<PathBuf>,
    current_index: usize,
    _stream: Option<OutputStream>,
    sink: Option<Arc<Mutex<Sink>>>,
    pub is_playing: bool,
    saved_positions: HashMap<usize, Duration>,
    pub shuffle: bool,
    track_duration: Option<Duration>,
}

impl MusicPlayer {
    pub fn new() -> Self {
        Self {
            files: Vec::new(),
            current_index: 0,
            _stream: None,
            sink: None,
            is_playing: false,
            saved_positions: HashMap::new(),
            shuffle: false,
            track_duration: None,
        }
    }

    pub fn load_dir(&mut self, target_dir: &Path) {
        if !target_dir.exists() {
            return;
        }
        let Ok(entries) = std::fs::read_dir(target_dir) else {
            return;
        };
        self.files = entries
            .filter_map(|e| {
                let p = e.ok()?.path();
                p.is_file().then_some(p)
            })
            .collect();
        self.files.sort();
    }

    pub fn current_file_is_video(&self) -> bool {
        let Some(path) = self.files.get(self.current_index) else {
            return false;
        };
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| VIDEO_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
            .unwrap_or(false)
    }

    pub fn current_file_path(&self) -> Option<&Path> {
        self.files.get(self.current_index).map(PathBuf::as_path)
    }

    /// Path of the next track without advancing the cursor. None if ≤1 tracks.
    pub fn next_file_path(&self) -> Option<&Path> {
        if self.files.len() < 2 {
            return None;
        }
        let idx = (self.current_index + 1) % self.files.len();
        Some(self.files[idx].as_path())
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    pub fn play(&mut self) {
        if self.files.is_empty() || self.current_file_is_video() {
            return;
        }
        if self.sink.is_none() {
            let Ok((stream, handle)) = OutputStream::try_default() else {
                return;
            };
            self._stream = Some(stream);
            let Ok(sink) = Sink::try_new(&handle) else {
                return;
            };
            let Ok(file) = File::open(&self.files[self.current_index]) else {
                return;
            };
            let Ok(src) = Decoder::new(BufReader::new(file)) else {
                return;
            };
            sink.append(src);
            // Seek to saved position before playing if we have one.
            if let Some(&pos) = self.saved_positions.get(&self.current_index) {
                let _ = sink.try_seek(pos);
            }
            sink.play();
            self.sink = Some(Arc::new(Mutex::new(sink)));
            self.track_duration = self.files.get(self.current_index)
                .and_then(|p| lofty::read_from_path(p).ok())
                .map(|tagged| tagged.properties().duration())
                .filter(|d| !d.is_zero());
        } else if let Some(ref sink) = self.sink {
            sink.lock().unwrap().play();
        }
        self.is_playing = true;
    }

    pub fn pause(&mut self) {
        if let Some(ref sink) = self.sink {
            let locked = sink.lock().unwrap();
            self.saved_positions.insert(self.current_index, locked.get_pos());
            locked.pause();
        }
        self.is_playing = false;
    }

    pub fn stop(&mut self) {
        if let Some(ref sink) = self.sink {
            let locked = sink.lock().unwrap();
            self.saved_positions.insert(self.current_index, locked.get_pos());
            locked.stop();
        }
        self.sink = None;
        self._stream = None;
        self.track_duration = None;
        self.is_playing = false;
    }

    pub fn track_progress(&self) -> Option<f32> {
        let duration = self.track_duration?;
        let sink = self.sink.as_ref()?;
        let pos = sink.lock().unwrap().get_pos();
        let total_secs = duration.as_secs_f32();
        if total_secs <= 0.0 {
            return None;
        }
        Some((pos.as_secs_f32() / total_secs).clamp(0.0, 1.0))
    }

    pub fn next_track(&mut self) {
        if self.files.is_empty() {
            return;
        }
        let was_playing = self.is_playing;
        let old_index = self.current_index;
        self.stop();
        self.saved_positions.remove(&old_index);
        if self.shuffle && self.files.len() > 1 {
            use rand::Rng;
            let mut rng = rand::thread_rng();
            let mut next = rng.gen_range(0..self.files.len());
            if next == old_index {
                next = (next + 1) % self.files.len();
            }
            self.current_index = next;
        } else {
            self.current_index = (self.current_index + 1) % self.files.len();
        }
        if was_playing && !self.current_file_is_video() {
            self.play();
        }
    }

    pub fn prev_track(&mut self) {
        if self.files.is_empty() {
            return;
        }
        let was_playing = self.is_playing;
        let old_index = self.current_index;
        self.stop();
        self.saved_positions.remove(&old_index);
        self.current_index = if self.current_index == 0 {
            self.files.len() - 1
        } else {
            self.current_index - 1
        };
        if was_playing && !self.current_file_is_video() {
            self.play();
        }
    }
}

/// One-shot audio player for countdown/bell sounds.
pub struct SoundPlayer {
    _stream: Option<OutputStream>,
    sink: Option<Sink>,
}

impl SoundPlayer {
    pub fn new() -> Self {
        Self { _stream: None, sink: None }
    }

    pub fn play(&mut self, path: &str) {
        if path.is_empty() {
            return;
        }
        let Ok((stream, handle)) = OutputStream::try_default() else { return };
        let Ok(sink) = Sink::try_new(&handle) else { return };
        let Ok(file) = File::open(path) else { return };
        let Ok(src) = Decoder::new(BufReader::new(file)) else { return };
        sink.append(src);
        sink.play();
        self._stream = Some(stream);
        self.sink = Some(sink);
    }

    pub fn is_done(&self) -> bool {
        self.sink.as_ref().map_or(true, |s| s.empty())
    }

    pub fn stop(&mut self) {
        if let Some(ref s) = self.sink {
            s.stop();
        }
        self.sink = None;
        self._stream = None;
    }
}
