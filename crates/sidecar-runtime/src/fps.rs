use std::time::{Duration, Instant};
#[derive(Debug, Default)]
pub struct FpsCounter { window: Duration, window_start: Option<Instant>, frames_in_window: u64, last_fps: f64 }
impl FpsCounter {
    pub fn new(window: Duration) -> Self { Self { window, ..Default::default() } }
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
    pub fn fps(&self) -> f64 { self.last_fps }
    pub fn stub_pulse(&mut self) -> f64 {
        if let Some(fps) = self.tick_frame() { fps } else { self.last_fps }
    }
}
#[cfg(test)]
mod tests {
    use super::*; use std::thread;
    #[test] fn reports_zero_before_first_window() { assert_eq!(FpsCounter::new(Duration::from_millis(50)).fps(), 0.0); }
    #[test] fn measures_frames_per_second_over_window() {
        let mut c = FpsCounter::new(Duration::from_millis(40));
        for _ in 0..8 { c.tick_frame(); }
        thread::sleep(Duration::from_millis(45));
        assert!(c.tick_frame().is_some());
        assert!(c.fps() > 10.0);
    }
}
