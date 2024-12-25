use rodio::{Decoder, OutputStream, Sink};
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub struct MusicPlayer {
    files: Vec<PathBuf>,
    current_index: usize,
    _stream: Option<OutputStream>,
    sink: Option<Arc<Mutex<Sink>>>,
    is_playing: bool,
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
        let paths = std::fs::read_dir(target_dir).unwrap();
        self.files = paths
            .filter_map(|entry| {
                let p = entry.unwrap().path();
                if p.is_file() {
                    Some(p)
                } else {
                    None
                }
            })
            .collect();
        println!("Loaded directory: {:?}", self.files);
    }

    pub fn play(&mut self) {
        if self.files.is_empty() {
            println!("No files to play.");
            return;
        }
        if self.sink.is_none() {
            let (stream, stream_handle) = OutputStream::try_default().unwrap();
            self._stream = Some(stream);
            let sink = Sink::try_new(&stream_handle).unwrap();
            let file = File::open(&self.files[self.current_index]).unwrap();
            let source = Decoder::new(BufReader::new(file)).unwrap();
            sink.append(source);
            sink.play();
            self.sink = Some(Arc::new(Mutex::new(sink)));
            self.is_playing = true;
            println!("Playing: {:?}", self.files[self.current_index]);
        } else {
            let sink = self.sink.as_ref().unwrap().lock().unwrap();
            sink.play();
            self.is_playing = true;
            println!("Resumed playing: {:?}", self.files[self.current_index]);
        }
    }

    pub fn pause(&mut self) {
        if let Some(ref sink) = self.sink {
            sink.lock().unwrap().pause();
            self.is_playing = false;
            println!("Paused: {:?}", self.files[self.current_index]);
        }
    }

    pub fn stop(&mut self) {
        if let Some(ref sink) = self.sink {
            sink.lock().unwrap().stop();
        }
        self.sink = None;
        self.is_playing = false;
        println!("Stopped playback.");
    }

    pub fn next_track(&mut self) {
        self.stop();
        self.current_index = (self.current_index + 1) % self.files.len();
        self.play();
    }

    pub fn prev_track(&mut self) {
        self.stop();
        if self.current_index == 0 {
            self.current_index = self.files.len() - 1;
        } else {
            self.current_index -= 1;
        }
        self.play();
    }
}
