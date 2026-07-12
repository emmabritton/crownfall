use std::time::Instant;

#[derive(Debug)]
pub struct Timing {
    /// amount of time that has passed since last
    pub delta: f64,
    /// when execution started
    pub started_at: Instant,
    /// time at start of frame
    pub now: Instant,
    /// time at start of last frame
    pub last: Instant,
    /// number of updates so far
    pub updates: usize,
    pub accumulated_time: f64,
    /// an fps independent value used to update animations, etc
    pub fixed_time_step: f64,
    /// an fps independent value used to update animations, etc
    pub fixed_time_step_f32: f32,
}

impl Timing {
    pub(crate) fn new(speed: usize) -> Timing {
        Timing {
            delta: 0.0,
            started_at: Instant::now(),
            now: Instant::now(),
            last: Instant::now(),
            accumulated_time: 0.0,
            updates: 0,
            fixed_time_step: 1.0 / (speed as f64),
            fixed_time_step_f32: 1.0 / (speed as f32),
        }
    }

    pub(crate) fn update(&mut self) {
        self.now = Instant::now();
        self.delta = self.now.duration_since(self.last).as_secs_f64();
        if self.delta > 0.1 {
            self.delta = 0.1;
        }
    }
}
