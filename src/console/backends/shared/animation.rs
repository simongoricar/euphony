use std::time::{Duration, Instant};

const PIXEL_SPINNER_PHASES: [char; 8] = ['⣾', '⣷', '⣯', '⣟', '⡿', '⢿', '⣻', '⣽'];
const PIE_SPINNER_PHASES: [char; 4] = ['◴', '◷', '◶', '◵'];
const SQUARE_SPINNER_PHASES: [char; 4] = ['◰', '◳', '◲', '◱'];
const ARC_SPINNER_PHASES: [char; 4] = ['◜', '◝', '◞', '◟'];
const MOON_SPINNER_PHASES: [char; 4] = ['◐', '◓', '◑', '◒'];
const PULSING_DOT_PHASES: [char; 3] = ['○', '◎', '●'];

#[derive(Copy, Clone)]
#[allow(dead_code)]
pub enum SpinnerStyle {
    Pixel,
    Pie,
    Square,
    Arc,
    Moon,
    PulsingDot,
}

pub fn get_spinner_phases(style: SpinnerStyle) -> &'static [char] {
    match style {
        SpinnerStyle::Pixel => &PIXEL_SPINNER_PHASES,
        SpinnerStyle::Pie => &PIE_SPINNER_PHASES,
        SpinnerStyle::Square => &SQUARE_SPINNER_PHASES,
        SpinnerStyle::Arc => &ARC_SPINNER_PHASES,
        SpinnerStyle::Moon => &MOON_SPINNER_PHASES,
        SpinnerStyle::PulsingDot => &PULSING_DOT_PHASES,
    }
}

pub fn get_spinner_default_speed(style: SpinnerStyle) -> Duration {
    match style {
        SpinnerStyle::Pixel => Duration::from_secs_f64(1.45),
        SpinnerStyle::Pie => Duration::from_secs_f64(2.0),
        SpinnerStyle::Square => Duration::from_secs_f64(2.0),
        SpinnerStyle::Arc => Duration::from_secs_f64(2.0),
        SpinnerStyle::Moon => Duration::from_secs_f64(2.0),
        SpinnerStyle::PulsingDot => Duration::from_secs_f64(2.2),
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct AnimatedSpinner {
    /// Time at which this spinner was started.
    init_time: Instant,
    
    /// A list of phases of this spiner.
    phases: &'static [char],
    
    /// How long each phase of the spinner should be held
    /// (automatically loops back to the first phase after the last one).
    phase_hold_time: Duration,
}

impl AnimatedSpinner {
    pub fn new(style: SpinnerStyle, speed: Option<Duration>) -> Self {
        let phases = get_spinner_phases(style);
        let speed = speed.unwrap_or_else(|| get_spinner_default_speed(style));
        
        let phase_hold_time = speed / phases.len() as u32;
        
        Self {
            init_time: Instant::now(),
            phases,
            phase_hold_time,
        }
    }
    
    pub fn get_current_phase(&self) -> char {
        let since_init = self.init_time
            .elapsed()
            .as_secs_f64();
        
        let phase_hold_time_secs = self.phase_hold_time
            .as_secs_f64();
        
        let current_index = (since_init / phase_hold_time_secs) as usize % self.phases.len();
        self.phases[current_index]
    }
}
