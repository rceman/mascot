//! DIP geometry primitives and pixel snapping.

/// A point in DIP space (origin = window client top-left, y down).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub fn new(x: f32, y: f32) -> Self {
        Point { x, y }
    }
}

/// An axis-aligned rectangle in DIP space.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Rect { x, y, w, h }
    }
    pub fn right(self) -> f32 {
        self.x + self.w
    }
    pub fn bottom(self) -> f32 {
        self.y + self.h
    }
    pub fn center(self) -> Point {
        Point::new(self.x + self.w / 2.0, self.y + self.h / 2.0)
    }
    pub fn contains(self, p: Point) -> bool {
        p.x >= self.x && p.x < self.right() && p.y >= self.y && p.y < self.bottom()
    }
    /// Expanded by `d` on every side.
    pub fn grow(self, d: f32) -> Rect {
        Rect::new(self.x - d, self.y - d, self.w + 2.0 * d, self.h + 2.0 * d)
    }
    /// Bounding union of two rects.
    pub fn union(self, o: Rect) -> Rect {
        let x = self.x.min(o.x);
        let y = self.y.min(o.y);
        Rect::new(
            x,
            y,
            self.right().max(o.right()) - x,
            self.bottom().max(o.bottom()) - y,
        )
    }
    /// Shrunk by `d` on every side.
    pub fn shrink(self, d: f32) -> Rect {
        self.grow(-d)
    }
    /// Mirrored horizontally about the vertical centre line of a `total_w`-wide area.
    pub fn mirror(self, total_w: f32) -> Rect {
        Rect::new(total_w - self.x - self.w, self.y, self.w, self.h)
    }
    /// True when `p` is inside the rect but outside its rounded corners of
    /// radius `r` (i.e. inside the corner square yet beyond the arc).
    pub fn in_cut_corner(self, p: Point, r: f32) -> bool {
        if !self.contains(p) {
            return false;
        }
        let r = r.min(self.w / 2.0).min(self.h / 2.0);
        let cx = if p.x < self.x + r {
            self.x + r
        } else if p.x > self.right() - r {
            self.right() - r
        } else {
            return false;
        };
        let cy = if p.y < self.y + r {
            self.y + r
        } else if p.y > self.bottom() - r {
            self.bottom() - r
        } else {
            return false;
        };
        (p.x - cx).powi(2) + (p.y - cy).powi(2) > r * r
    }
}

/// Snaps a DIP coordinate to the device pixel grid for `scale` (px = dip*scale).
///
/// Used for 1-DIP borders and other hairlines so they land exactly on device
/// pixels at 100/125/150/200 %.
pub fn snap(dip: f32, scale: f32) -> f32 {
    (dip * scale).round() / scale
}

/// Snaps a rect's edges to the device grid.
pub fn snap_rect(r: Rect, scale: f32) -> Rect {
    let x = snap(r.x, scale);
    let y = snap(r.y, scale);
    Rect::new(
        x,
        y,
        snap(r.right(), scale) - x,
        snap(r.bottom(), scale) - y,
    )
}
