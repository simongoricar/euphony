use std::time::{Duration, Instant};

// Phases for each variant of the animated spinner.
const PIXEL_SPINNER_PHASES: [char; 8] = ['⣾', '⣷', '⣯', '⣟', '⡿', '⢿', '⣻', '⣽'];
const PIE_SPINNER_PHASES: [char; 4] = ['◴', '◷', '◶', '◵'];
const SQUARE_SPINNER_PHASES: [char; 4] = ['◰', '◳', '◲', '◱'];
const ARC_SPINNER_PHASES: [char; 6] = ['◜', '◠', '◝', '◞', '◡', '◟'];
const MOON_SPINNER_PHASES: [char; 4] = ['◐', '◓', '◑', '◒'];
const PULSING_DOT_PHASES: [char; 3] = ['○', '◎', '●'];

/// All available animated spinner styles. See top of `animation.rs` for the phases of each.
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

impl SpinnerStyle {
    /// Get the phases (`[char]`) associated with the given spinner style.
    fn get_phases(&self) -> &'static [char] {
        match self {
            SpinnerStyle::Pixel => &PIXEL_SPINNER_PHASES,
            SpinnerStyle::Pie => &PIE_SPINNER_PHASES,
            SpinnerStyle::Square => &SQUARE_SPINNER_PHASES,
            SpinnerStyle::Arc => &ARC_SPINNER_PHASES,
            SpinnerStyle::Moon => &MOON_SPINNER_PHASES,
            SpinnerStyle::PulsingDot => &PULSING_DOT_PHASES,
        }
    }

    /// Get the default speed associated with the given spinner style.
    fn get_default_duration(&self) -> Duration {
        match self {
            SpinnerStyle::Pixel => Duration::from_secs_f64(1.45),
            SpinnerStyle::Pie => Duration::from_secs_f64(2.0),
            SpinnerStyle::Square => Duration::from_secs_f64(2.0),
            SpinnerStyle::Arc => Duration::from_secs_f64(1.5),
            SpinnerStyle::Moon => Duration::from_secs_f64(2.0),
            SpinnerStyle::PulsingDot => Duration::from_secs_f64(2.2),
        }
    }
}


/// Generic animated spinner implementation. Can handle any simple phase-based animation.
///
/// The user provides:
/// - a list of chars that each represent a phase (a state),
/// - the hold time for a single phase (speed of the animation).
#[derive(Clone, Eq, PartialEq)]
pub struct AnimatedSpinner {
    /// Time at which this spinner was started.
    init_time: Instant,

    /// A list of phases of this spinner.
    phases: &'static [char],

    /// How long each phase of the spinner should be held
    /// (automatically loops back to the first phase after the last one).
    phase_hold_time: Duration,
}

impl AnimatedSpinner {
    /// Initialize a new `AnimatedSpinner` by providing a `SpinnerStyle` and the `speed` at which
    /// it should be played. If `speed` is `None`, a default speed associated with the selected
    /// spinner style is used.
    pub fn new(style: SpinnerStyle, loop_duration: Option<Duration>) -> Self {
        let phases = style.get_phases();
        let animation_duration =
            loop_duration.unwrap_or_else(|| style.get_default_duration());

        let phase_hold_time = animation_duration / phases.len() as u32;

        Self {
            init_time: Instant::now(),
            phases,
            phase_hold_time,
        }
    }

    /// Get the current `char` to display (use when rendering).
    ///
    /// NOTE: This is a clock time-sensitive method! If, for example, `speed` was `10 ms`,
    /// this method might, at one point in time, return `◴`. After `10 ms`, it will start returning
    /// the next phase (`◷`), holding it for the same amount, and so on.
    /// After it encounters the last phase it will loop back to the start.
    pub fn get_current_phase(&self) -> char {
        let since_init = self.init_time.elapsed().as_secs_f64();

        let phase_hold_time_secs = self.phase_hold_time.as_secs_f64();

        let current_index =
            (since_init / phase_hold_time_secs) as usize % self.phases.len();
        self.phases[current_index]
    }
}
