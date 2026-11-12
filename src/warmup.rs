//! A guided one-minute warm-up before the first drill of the day: a few slow
//! breaths to settle in, then gentle hand and wrist stretches.

use std::time::{Duration, Instant};

pub struct Step {
    pub title: &'static str,
    pub how: &'static str,
    pub secs: u64,
}

pub const STEPS: [Step; 6] = [
    Step {
        title: "Breathe",
        how: "Slow breaths: in for 4, out for 6. Drop your shoulders on every out-breath.",
        secs: 20,
    },
    Step {
        title: "Fist and fan",
        how: "Make a loose fist, then spread your fingers wide. Five slow times.",
        secs: 10,
    },
    Step {
        title: "Prayer stretch",
        how: "Palms together at your chest. Lower them slowly until your wrists feel a gentle stretch, and hold.",
        secs: 10,
    },
    Step {
        title: "Wrist circles",
        how: "Five slow circles each way.",
        secs: 10,
    },
    Step {
        title: "Thumb touches",
        how: "Touch your thumb to each fingertip and back again, both hands.",
        secs: 8,
    },
    Step {
        title: "Shake it out",
        how: "Let your hands hang loose and shake them out.",
        secs: 5,
    },
];

pub struct Warmup {
    started: Instant,
}

impl Warmup {
    pub fn new(now: Instant) -> Warmup {
        Warmup { started: now }
    }

    fn elapsed(&self, now: Instant) -> Duration {
        now.saturating_duration_since(self.started)
    }

    /// The step you're on and the seconds left in it, or `None` when done.
    pub fn current(&self, now: Instant) -> Option<(usize, u64)> {
        let mut t = self.elapsed(now).as_secs_f64();
        for (i, step) in STEPS.iter().enumerate() {
            if t < step.secs as f64 {
                return Some((i, (step.secs as f64 - t).ceil() as u64));
            }
            t -= step.secs as f64;
        }
        None
    }

    pub fn done(&self, now: Instant) -> bool {
        self.current(now).is_none()
    }

    /// During the breathing step: whether to breathe in or out right now.
    pub fn breath(&self, now: Instant) -> Option<&'static str> {
        let (step, _) = self.current(now)?;
        if step != 0 {
            return None;
        }
        let cycle = self.elapsed(now).as_secs_f64() % 10.0;
        Some(if cycle < 4.0 {
            "breathe in…"
        } else {
            "and out…"
        })
    }

    /// Jumps to the start of the next step.
    pub fn skip_step(&mut self, now: Instant) {
        if let Some((_, left)) = self.current(now) {
            let left = Duration::from_secs(left);
            self.started = self.started.checked_sub(left).unwrap_or(self.started);
        }
    }

    pub fn total() -> u64 {
        STEPS.iter().map(|s| s.secs).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walks_through_the_steps_in_about_a_minute() {
        let t0 = Instant::now();
        let w = Warmup::new(t0);
        assert_eq!(w.current(t0), Some((0, 20)));
        assert_eq!(w.current(t0 + Duration::from_secs(21)), Some((1, 9)));
        assert!((55..=70).contains(&Warmup::total()));
        assert!(w.done(t0 + Duration::from_secs(Warmup::total())));
    }

    #[test]
    fn breathes_in_for_four_and_out_for_six() {
        let t0 = Instant::now();
        let w = Warmup::new(t0);
        assert_eq!(w.breath(t0 + Duration::from_secs(2)), Some("breathe in…"));
        assert_eq!(w.breath(t0 + Duration::from_secs(7)), Some("and out…"));
        assert_eq!(w.breath(t0 + Duration::from_secs(25)), None);
    }

    #[test]
    fn enter_skips_to_the_next_step() {
        let t0 = Instant::now() + Duration::from_secs(100);
        let mut w = Warmup::new(t0);
        w.skip_step(t0 + Duration::from_secs(3));
        assert_eq!(w.current(t0 + Duration::from_secs(3)).map(|c| c.0), Some(1));
    }
}
