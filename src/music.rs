use rodio::{Decoder, OutputStream, Sink};
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const VIDEO_EXTENSIONS: &[&str] = &[
    "mp4", "mkv", "webm", "avi", "mov", "m4v", "wmv", "flv", "ts", "m2ts",
];

pub struct MusicPlayer {
    files: Vec<PathBuf>,
    current_index: usize,
    _stream: Option<OutputStream>,
    sink: Option<Arc<Mutex<Sink>>>,
    pub is_playing: bool,
}

impl MusicPlayer {
    pub fn new() -> Self {
        Self {
            files: Vec::new(),
            current_index: 0,
            _stream: None,
            sink: None,
            is_playing: false,
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
            sink.play();
            self.sink = Some(Arc::new(Mutex::new(sink)));
        } else if let Some(ref sink) = self.sink {
            sink.lock().unwrap().play();
        }
        self.is_playing = true;
    }

    pub fn pause(&mut self) {
        if let Some(ref sink) = self.sink {
            sink.lock().unwrap().pause();
        }
        self.is_playing = false;
    }

    pub fn stop(&mut self) {
        if let Some(ref sink) = self.sink {
            sink.lock().unwrap().stop();
        }
        self.sink = None;
        self._stream = None;
        self.is_playing = false;
    }

    pub fn next_track(&mut self) {
        if self.files.is_empty() {
            return;
        }
        let was_playing = self.is_playing;
        self.stop();
        self.current_index = (self.current_index + 1) % self.files.len();
        if was_playing && !self.current_file_is_video() {
            self.play();
        }
    }

    pub fn prev_track(&mut self) {
        if self.files.is_empty() {
            return;
        }
        let was_playing = self.is_playing;
        self.stop();
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
