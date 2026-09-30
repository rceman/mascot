//! Pure motion model (no Win32): shadcn/Tailwind v4 transition + tw-animate-css
//! tooltip keyframes. Time is an explicit `now_ms: f64`; painters and the app
//! layer consume `ControlColors` / `TooltipFrame` snapshots.

pub const TRANSITION_MS: f64 = 150.0; // Tailwind v4 default transition
pub const TOOLTIP_ANIM_MS: f64 = 150.0; // tw-animate-css animate-in/out
pub const TOOLTIP_SCALE_FROM: f32 = 0.95; // zoom-in-95 / zoom-out-95
pub const TOOLTIP_SLIDE_DIP: f32 = 8.0; // slide-in-from-bottom-2, enter only

/// Solve cubic-bezier(x1,y1,x2,y2)(x) for x in [0,1]: Newton + bisection
/// fallback on the parameter s such that x(s) = x, then return y(s).
pub fn cubic_bezier(x1: f32, y1: f32, x2: f32, y2: f32, x: f32) -> f32 {
    if x <= 0.0 {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }
    let eval = |t: f32, a: f32, b: f32| -> f32 {
        // B(t) = 3(1-t)^2 t a + 3(1-t) t^2 b + t^3  (endpoints 0/1)
        let u = 1.0 - t;
        3.0 * u * u * t * a + 3.0 * u * t * t * b + t * t * t
    };
    let deriv = |t: f32, a: f32, b: f32| -> f32 {
        let u = 1.0 - t;
        3.0 * u * u * a + 6.0 * u * t * (b - a) + 3.0 * t * t * (1.0 - b)
    };
    // Newton
    let mut s = x;
    for _ in 0..8 {
        let err = eval(s, x1, x2) - x;
        if err.abs() < 1e-6 {
            return eval(s, y1, y2);
        }
        let d = deriv(s, x1, x2);
        if d.abs() < 1e-6 {
            break;
        }
        s = (s - err / d).clamp(0.0, 1.0);
    }
    // bisection fallback
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    for _ in 0..40 {
        let mid = (lo + hi) / 2.0;
        if eval(mid, x1, x2) < x {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    eval((lo + hi) / 2.0, y1, y2)
}

/// Tailwind v4 `--default-transition-timing-function`.
pub fn ease_standard(x: f32) -> f32 {
    cubic_bezier(0.4, 0.0, 0.2, 1.0, x)
}

/// tw-animate-css `ease` = cubic-bezier(0.25, 0.1, 0.25, 1).
pub fn ease_css(x: f32) -> f32 {
    cubic_bezier(0.25, 0.1, 0.25, 1.0, x)
}

/// Animated per-control paint values. `ring` = focus-ring progress 0..=1.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ControlColors {
    pub fill: [f32; 4],
    pub fg: [f32; 4],
    pub ring: f32,
}

/// CSS colour interpolation: premultiplied RGBA blend, then un-premultiply.
pub fn mix_premul(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    let (pa, pb) = (a[3], b[3]);
    let out_a = pa + (pb - pa) * t;
    if out_a <= 0.0 {
        return [0.0; 4];
    }
    let mut out = [0.0f32; 4];
    for i in 0..3 {
        let pa_c = a[i] * pa;
        let pb_c = b[i] * pb;
        out[i] = (pa_c + (pb_c - pa_c) * t) / out_a;
    }
    out[3] = out_a;
    out
}

/// 150 ms colour/ring tween between two `ControlColors` snapshots.
#[derive(Clone, Copy, Debug)]
pub struct Tween {
    from: ControlColors,
    to: ControlColors,
    start_ms: f64,
}

impl Tween {
    /// A tween already at `c` (no animation).
    pub fn settled(c: ControlColors) -> Self {
        Tween {
            from: c,
            to: c,
            start_ms: f64::NEG_INFINITY,
        }
    }

    /// Current value. `reduced` or elapsed >= TRANSITION_MS returns `to`
    /// bit-identically (no interpolation error at the endpoint).
    pub fn value(&self, now_ms: f64, reduced: bool) -> ControlColors {
        let elapsed = now_ms - self.start_ms;
        if reduced || elapsed >= TRANSITION_MS {
            return self.to;
        }
        if elapsed <= 0.0 {
            return self.from;
        }
        let p = ease_standard((elapsed / TRANSITION_MS) as f32);
        ControlColors {
            fill: mix_premul(self.from.fill, self.to.fill, p),
            fg: mix_premul(self.from.fg, self.to.fg, p),
            ring: self.from.ring + (self.to.ring - self.from.ring) * p,
        }
    }

    /// Start (or re-aim) a transition to `to`. No-op if already heading there;
    /// otherwise `from` is the current interpolated value (mid-flight retarget).
    pub fn retarget(&mut self, to: ControlColors, now_ms: f64, reduced: bool) {
        if to == self.to {
            return;
        }
        self.from = self.value(now_ms, reduced);
        self.to = to;
        self.start_ms = now_ms;
    }

    pub fn active(&self, now_ms: f64, reduced: bool) -> bool {
        !reduced && now_ms - self.start_ms < TRANSITION_MS && self.from != self.to
    }
}

/// Tooltip transform snapshot. `IDENTITY` = fully open (no transform).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct TooltipFrame {
    pub opacity: f32,
    pub scale: f32,
    pub dy: f32,
}

pub const TOOLTIP_IDENTITY: TooltipFrame = TooltipFrame {
    opacity: 1.0,
    scale: 1.0,
    dy: 0.0,
};

#[derive(Clone, Copy, Debug)]
pub enum TooltipPhase {
    Opening { start_ms: f64 },
    Open,
    Closing { start_ms: f64, from: TooltipFrame },
}

/// animate-in fade-in-0 zoom-in-95 slide-in-from-bottom-2:
/// opacity p, scale .95+.05p, dy +8*(1-p) DIP (slides up into place).
pub fn tooltip_open_frame(elapsed_ms: f64) -> TooltipFrame {
    let p = ease_css((elapsed_ms / TOOLTIP_ANIM_MS).clamp(0.0, 1.0) as f32);
    TooltipFrame {
        opacity: p,
        scale: TOOLTIP_SCALE_FROM + (1.0 - TOOLTIP_SCALE_FROM) * p,
        dy: TOOLTIP_SLIDE_DIP * (1.0 - p),
    }
}

/// animate-out fade-out-0 zoom-out-95: lerp `from` -> {0, .95, from.dy}
/// eased with ease_css. Exit has no slide (per shadcn).
pub fn tooltip_close_frame(from: TooltipFrame, elapsed_ms: f64) -> TooltipFrame {
    let p = ease_css((elapsed_ms / TOOLTIP_ANIM_MS).clamp(0.0, 1.0) as f32);
    TooltipFrame {
        opacity: from.opacity * (1.0 - p),
        scale: from.scale + (TOOLTIP_SCALE_FROM - from.scale) * p,
        dy: from.dy,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn easing_endpoints_and_monotonic() {
        for ease in [ease_standard, ease_css] {
            assert_eq!(ease(0.0), 0.0);
            assert_eq!(ease(1.0), 1.0);
            let mut prev = -1.0f32;
            for i in 0..=1000 {
                let v = ease(i as f32 / 1000.0);
                assert!(v >= prev - 1e-6, "ease not monotonic at {}", i);
                prev = v;
            }
        }
        assert!((ease_css(0.5) - 0.8024).abs() < 1e-3);
    }

    #[test]
    fn mix_premul_from_transparent() {
        let f5 = rgb(0xf5f5f5);
        let m = mix_premul([0.0; 4], f5, 0.5);
        assert!((m[3] - 0.5).abs() < 1e-6);
        for i in 0..3 {
            assert!((m[i] - f5[i]).abs() < 1e-4, "channel {i}: {}", m[i]);
        }
    }

    fn rgb(h: u32) -> [f32; 4] {
        let (r, g, b) = ((h >> 16) & 0xff, (h >> 8) & 0xff, h & 0xff);
        [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0]
    }

    fn c(fill: u32, ring: f32) -> ControlColors {
        ControlColors {
            fill: rgb(fill),
            fg: rgb(fill),
            ring,
        }
    }

    #[test]
    fn tween_lifecycle() {
        let a = c(0x111111, 0.0);
        let b = c(0x222222, 1.0);
        let mut tw = Tween::settled(a);
        assert!(!tw.active(0.0, false));
        tw.retarget(b, 100.0, false);
        assert_eq!(tw.value(100.0, false).fill, a.fill); // t=0 -> from
        assert_eq!(tw.value(250.0, false), b); // elapsed >= 150: bit-identical
        assert_eq!(tw.value(120.0, true), b); // reduced snaps
        let mid = tw.value(175.0, false);
        assert!(mid.fill[0] > a.fill[0] && mid.fill[0] < b.fill[0]);
        // retarget mid-flight starts from the current value
        tw.retarget(a, 175.0, false);
        assert_eq!(tw.value(175.0, false), mid);
        // retarget to same target is a no-op
        let same = c(0x222222, 1.0);
        let mut tw3 = Tween::settled(a);
        tw3.retarget(same, 0.0, false);
        tw3.retarget(same, 50.0, false); // still heading to same `to`
        assert_eq!(tw3.to, same);
    }

    #[test]
    fn tooltip_frames() {
        let f0 = tooltip_open_frame(0.0);
        assert_eq!(f0.opacity, 0.0);
        assert_eq!(f0.scale, TOOLTIP_SCALE_FROM);
        assert_eq!(f0.dy, TOOLTIP_SLIDE_DIP);
        let f150 = tooltip_open_frame(150.0);
        assert_eq!(f150, TOOLTIP_IDENTITY);
        let fc0 = tooltip_close_frame(TOOLTIP_IDENTITY, 0.0);
        assert_eq!(fc0, TOOLTIP_IDENTITY);
        let fc150 = tooltip_close_frame(TOOLTIP_IDENTITY, 150.0);
        assert_eq!(fc150.opacity, 0.0);
        assert_eq!(fc150.scale, TOOLTIP_SCALE_FROM);
    }
}
