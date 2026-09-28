//! One-shot idle scheduling (the v0.2 seed of the product IdleDirector).
//!
//! The director never ticks. The host asks for the next wake-up delay, arms a
//! single timer, and on expiry calls [`IdleDirector::choose`] to get one clip to
//! play. Between clips the mascot is fully static.

#[derive(Debug, Clone)]
pub struct IdleAction {
    pub clip: usize,
    pub weight: f32,
    /// Minimum seconds before this action may repeat.
    pub cooldown: f32,
}

#[derive(Debug, Clone)]
pub struct IdleDirector {
    actions: Vec<IdleAction>,
    last_played_at: Vec<f32>,
    last: Option<usize>,
    rng: u64,
    /// Wake-up delay range in seconds.
    pub delay_range: (f32, f32),
    /// Contextual suppression (e.g. user is typing / window hidden).
    pub suppressed: bool,
}

impl IdleDirector {
    pub fn new(actions: Vec<IdleAction>, seed: u64) -> Self {
        let n = actions.len();
        IdleDirector {
            actions,
            last_played_at: vec![f32::NEG_INFINITY; n],
            last: None,
            rng: seed.max(1),
            delay_range: (4.0, 11.0),
            suppressed: false,
        }
    }

    fn next_f32(&mut self) -> f32 {
        // xorshift64*
        self.rng ^= self.rng >> 12;
        self.rng ^= self.rng << 25;
        self.rng ^= self.rng >> 27;
        let v = self.rng.wrapping_mul(0x2545_F491_4F6C_DD1D);
        (v >> 40) as f32 / (1u64 << 24) as f32
    }

    /// Randomised delay until the next wake-up (seconds).
    pub fn next_delay(&mut self) -> f32 {
        let (a, b) = self.delay_range;
        a + (b - a) * self.next_f32()
    }

    /// Weighted pick among actions that are off cooldown and not an immediate repeat.
    pub fn choose(&mut self, now: f32) -> Option<usize> {
        if self.suppressed {
            return None;
        }
        let eligible: Vec<usize> = (0..self.actions.len())
            .filter(|&i| Some(i) != self.last || self.actions.len() == 1)
            .filter(|&i| now - self.last_played_at[i] >= self.actions[i].cooldown)
            .collect();
        let total: f32 = eligible.iter().map(|&i| self.actions[i].weight).sum();
        if total <= 0.0 {
            return None;
        }
        let mut r = self.next_f32() * total;
        let mut pick = eligible[eligible.len() - 1];
        for &i in &eligible {
            r -= self.actions[i].weight;
            if r <= 0.0 {
                pick = i;
                break;
            }
        }
        self.last = Some(pick);
        self.last_played_at[pick] = now;
        Some(self.actions[pick].clip)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn director() -> IdleDirector {
        IdleDirector::new(
            vec![
                IdleAction { clip: 10, weight: 5.0, cooldown: 0.0 },
                IdleAction { clip: 11, weight: 1.0, cooldown: 30.0 },
                IdleAction { clip: 12, weight: 1.0, cooldown: 0.0 },
            ],
            42,
        )
    }

    #[test]
    fn deterministic_no_immediate_repeat_and_cooldowns() {
        let mut a = director();
        let mut b = director();
        let mut t = 0.0;
        let mut prev = None;
        let mut seen_11 = Vec::new();
        for _ in 0..200 {
            t += a.next_delay();
            let _ = b.next_delay();
            let ca = a.choose(t);
            assert_eq!(ca, b.choose(t));
            assert_ne!(ca, prev);
            if ca == Some(11) {
                seen_11.push(t);
            }
            prev = ca;
        }
        assert!(seen_11.windows(2).all(|w| w[1] - w[0] >= 30.0));
    }

    #[test]
    fn delays_in_range_and_suppression() {
        let mut d = director();
        for _ in 0..100 {
            let v = d.next_delay();
            assert!((4.0..=11.0).contains(&v));
        }
        d.suppressed = true;
        assert_eq!(d.choose(100.0), None);
    }
}
