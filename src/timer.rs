pub(crate) struct Timer {
    start_time: std::time::Instant,
    current_time: std::time::Instant,
    duration: u64,
    is_running: bool,
    paused_time: Option<std::time::Instant>,
}

impl Timer {
    pub fn new(duration: u64) -> Self {
        Timer {
            start_time: std::time::Instant::now(),
            current_time: std::time::Instant::now(),
            duration,
            is_running: false,
            paused_time: None,
        }
    }

    pub fn start(&mut self) {
        if let Some(paused_time) = self.paused_time {
            let elapsed = paused_time.elapsed().as_secs_f64();
            self.start_time += std::time::Duration::from_secs_f64(elapsed);
        } else {
            self.start_time = std::time::Instant::now();
        }
        self.paused_time = None;
        self.is_running = true;
    }

    pub fn pause(&mut self) {
        self.is_running = false;
        self.paused_time = Some(std::time::Instant::now());
    }

    pub fn reset(&mut self) {
        self.start_time = std::time::Instant::now();
        self.is_running = false;
        self.paused_time = None;
    }

    pub fn check_time(&self) -> f64 {
        if self.is_running {
            if let Some(paused_time) = self.paused_time {
                let elapsed = paused_time.elapsed().as_secs_f64();
                if elapsed >= self.duration as f64 {
                    return 0.0;
                } else {
                    return self.duration as f64 + paused_time.elapsed().as_secs_f64()
                        - self.start_time.elapsed().as_secs_f64();
                }
            }
            let elapsed = self.start_time.elapsed().as_secs_f64();
            if elapsed >= self.duration as f64 {
                return 0.0;
            } else {
                return self.duration as f64 - elapsed;
            }
        } else {
            return self.duration as f64;
        }
    }
    pub fn total_time(&self) -> u64 {
        self.duration
    }
}
