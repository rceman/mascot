//! 2D affine transforms.
//!
//! Convention (matches Direct2D `Matrix3x2` with m11=a, m12=b, m21=c, m22=d):
//!
//! ```text
//! x' = a*x + c*y + tx
//! y' = b*x + d*y + ty
//! ```
//!
//! Canvas space is y-down, so a positive rotation turns clockwise on screen.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Affine {
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
    pub tx: f32,
    pub ty: f32,
}

impl Default for Affine {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Affine {
    pub const IDENTITY: Affine = Affine { a: 1.0, b: 0.0, c: 0.0, d: 1.0, tx: 0.0, ty: 0.0 };

    pub fn translate(x: f32, y: f32) -> Self {
        Affine { tx: x, ty: y, ..Self::IDENTITY }
    }

    pub fn scale(sx: f32, sy: f32) -> Self {
        Affine { a: sx, d: sy, ..Self::IDENTITY }
    }

    pub fn rotate_deg(deg: f32) -> Self {
        let (s, c) = deg.to_radians().sin_cos();
        Affine { a: c, b: s, c: -s, d: c, tx: 0.0, ty: 0.0 }
    }

    /// `self * rhs`: applies `rhs` first, then `self`.
    pub fn mul(self, rhs: Affine) -> Affine {
        Affine {
            a: self.a * rhs.a + self.c * rhs.b,
            b: self.b * rhs.a + self.d * rhs.b,
            c: self.a * rhs.c + self.c * rhs.d,
            d: self.b * rhs.c + self.d * rhs.d,
            tx: self.a * rhs.tx + self.c * rhs.ty + self.tx,
            ty: self.b * rhs.tx + self.d * rhs.ty + self.ty,
        }
    }

    pub fn apply(self, p: [f32; 2]) -> [f32; 2] {
        [self.a * p[0] + self.c * p[1] + self.tx, self.b * p[0] + self.d * p[1] + self.ty]
    }

    pub fn det(self) -> f32 {
        self.a * self.d - self.b * self.c
    }

    pub fn inverse(self) -> Option<Affine> {
        let det = self.det();
        if det.abs() < 1e-12 {
            return None;
        }
        let inv = 1.0 / det;
        let a = self.d * inv;
        let b = -self.b * inv;
        let c = -self.c * inv;
        let d = self.a * inv;
        Some(Affine { a, b, c, d, tx: -(a * self.tx + c * self.ty), ty: -(b * self.tx + d * self.ty) })
    }

    /// Scales every coefficient; used for linear-blend skinning.
    pub fn weighted(self, w: f32) -> Affine {
        Affine { a: self.a * w, b: self.b * w, c: self.c * w, d: self.d * w, tx: self.tx * w, ty: self.ty * w }
    }

    pub fn add(self, o: Affine) -> Affine {
        Affine {
            a: self.a + o.a,
            b: self.b + o.b,
            c: self.c + o.c,
            d: self.d + o.d,
            tx: self.tx + o.tx,
            ty: self.ty + o.ty,
        }
    }

    pub fn approx_eq(self, o: Affine, eps: f32) -> bool {
        [self.a - o.a, self.b - o.b, self.c - o.c, self.d - o.d, self.tx - o.tx, self.ty - o.ty]
            .iter()
            .all(|v| v.abs() <= eps)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positive_rotation_is_clockwise_on_screen() {
        // +x axis rotated +90deg must point down (+y) in a y-down canvas.
        let p = Affine::rotate_deg(90.0).apply([1.0, 0.0]);
        assert!((p[0]).abs() < 1e-6 && (p[1] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn mul_applies_right_operand_first() {
        let m = Affine::translate(10.0, 0.0).mul(Affine::scale(2.0, 2.0));
        assert_eq!(m.apply([1.0, 1.0]), [12.0, 2.0]);
    }

    #[test]
    fn inverse_round_trips() {
        let m = Affine::translate(3.0, -7.0).mul(Affine::rotate_deg(33.0)).mul(Affine::scale(-1.5, 0.5));
        let r = m.mul(m.inverse().unwrap());
        assert!(r.approx_eq(Affine::IDENTITY, 1e-5));
    }
}
