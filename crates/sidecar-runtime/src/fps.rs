use std::time::{Duration, Instant};

#[derive(Debug, Default)]
pub struct FpsCounter {
    window: Duration,
    window_start: Option<Instant>,
    frames_in_window: u64,
    last_fps: f64,
}

impl FpsCounter {
    pub fn new(window: Duration) -> Self {
        Self {
            window,
            ..Default::default()
        }
    }

    pub fn tick_frame(&mut self) -> Option<f64> {
        let now = Instant::now();
        let start = *self.window_start.get_or_insert(now);
        self.frames_in_window += 1;
        let elapsed = now.duration_since(start);
        if elapsed >= self.window {
            self.last_fps = self.frames_in_window as f64 / elapsed.as_secs_f64();
            self.frames_in_window = 0;
            self.window_start = Some(now);
            return Some(self.last_fps);
        }
        None
    }

    pub fn fps(&self) -> f64 {
        self.last_fps
    }

    pub fn stub_pulse(&mut self) -> f64 {
        let _ = self.tick_frame();
        if self.last_fps == 0.0 {
            self.last_fps = 60.0;
        }
        self.last_fps
    }
}
