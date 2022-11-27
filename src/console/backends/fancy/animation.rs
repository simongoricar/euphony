use std::ops::Div;
use std::time::{Duration, Instant};

const PIXEL_SPINNER_PHASES: [char; 8] = ['⣾', '⣷', '⣯', '⣟', '⡿', '⢿', '⣻', '⣽'];

#[derive(Clone, Eq, PartialEq)]
pub struct PixelSpinner {
    init_time: Instant,
    phase_hold_time: Duration,
}

impl PixelSpinner {
    pub fn new(spin_speed: Option<Duration>) -> Self {
        let phase_hold_time = spin_speed
            .unwrap_or(Duration::from_secs(1))
            .div(PIXEL_SPINNER_PHASES.len() as u32);
        
        Self {
            init_time: Instant::now(),
            phase_hold_time,
        }
    }
    
    pub fn get_current_phase(&self) -> char {
        let since_init = self.init_time
            .elapsed()
            .as_secs_f64();
        
        let phase_hold_time_secs = self.phase_hold_time
            .as_secs_f64();
        
        let current_index = (since_init / phase_hold_time_secs) as usize % PIXEL_SPINNER_PHASES.len();
        PIXEL_SPINNER_PHASES[current_index]
    }
}
