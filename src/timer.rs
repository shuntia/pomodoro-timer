use tracing::debug;

pub struct Timer {
    start_time: std::time::Instant,
    duration: u64,
    is_running: bool,
    paused_at: Option<std::time::Instant>,
}

impl Timer {
    pub fn new(duration: u64) -> Self {
        Timer {
            start_time: std::time::Instant::now(),
            duration,
            is_running: false,
            paused_at: None,
        }
    }

    pub fn start(&mut self) {
        debug!(duration = self.duration, "Timer::start called");
        if let Some(paused_time) = self.paused_at {
            // Shift start_time forward by pause duration so elapsed stays correct
            let pause_len = paused_time.elapsed();
            self.start_time += pause_len;
        } else {
            self.start_time = std::time::Instant::now();
        }
        self.paused_at = None;
        self.is_running = true;
    }

    pub fn pause(&mut self) {
        self.is_running = false;
        self.paused_at = Some(std::time::Instant::now());
    }

    pub fn reset(&mut self) {
        self.start_time = std::time::Instant::now();
        self.is_running = false;
        self.paused_at = None;
    }

    pub fn is_running(&self) -> bool {
        self.is_running
    }

    pub fn is_idle(&self) -> bool {
        !self.is_running && self.paused_at.is_none()
    }

    pub fn check_time(&self) -> f64 {
        let elapsed = if self.is_running {
            self.start_time.elapsed().as_secs_f64()
        } else if let Some(paused_at) = self.paused_at {
            // Time elapsed from (adjusted) start to when we paused
            paused_at
                .saturating_duration_since(self.start_time)
                .as_secs_f64()
        } else {
            return self.duration as f64;
        };

        (self.duration as f64 - elapsed).max(0.0)
    }

    pub fn total_time(&self) -> u64 {
        self.duration
    }
}
