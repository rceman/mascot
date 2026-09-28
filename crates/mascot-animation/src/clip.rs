//! Keyframed animation clips.
//!
//! Track values are *offsets from rest*: `x`, `y` (pixels, parent-local) and
//! `rotation` (degrees) add; `scale_x`, `scale_y` and `opacity` multiply (1 = rest).
//! Non-looping clips must end at the neutral value so completion returns to rest.

use crate::rig::RigError;
use crate::skeleton::{Pose, Skeleton};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Ease {
    Linear,
    Step,
    InQuad,
    OutQuad,
    InOutQuad,
    InOutCubic,
    InOutSine,
    OutBack,
}

impl Ease {
    pub fn apply(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Ease::Linear => t,
            Ease::Step => {
                if t < 1.0 {
                    0.0
                } else {
                    1.0
                }
            }
            Ease::InQuad => t * t,
            Ease::OutQuad => 1.0 - (1.0 - t) * (1.0 - t),
            Ease::InOutQuad => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
                }
            }
            Ease::InOutCubic => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
                }
            }
            Ease::InOutSine => -((std::f32::consts::PI * t).cos() - 1.0) / 2.0,
            Ease::OutBack => {
                let c1 = 1.70158;
                let c3 = c1 + 1.0;
                1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Property {
    X,
    Y,
    Rotation,
    ScaleX,
    ScaleY,
    /// Targets a part (all its attachments), not a bone.
    Opacity,
}

impl Property {
    pub fn neutral(self) -> f32 {
        match self {
            Property::X | Property::Y | Property::Rotation => 0.0,
            Property::ScaleX | Property::ScaleY | Property::Opacity => 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Key {
    /// Seconds from clip start.
    pub t: f32,
    pub v: f32,
    /// Easing used from this key to the next.
    #[serde(default = "default_ease")]
    pub ease: Ease,
}

fn default_ease() -> Ease {
    Ease::InOutSine
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
    /// Bone id (or part id for `opacity`).
    pub target: String,
    pub property: Property,
    pub keys: Vec<Key>,
}

impl Track {
    pub fn sample(&self, t: f32) -> f32 {
        let keys = &self.keys;
        let Some(first) = keys.first() else { return self.property.neutral() };
        if t <= first.t {
            return first.v;
        }
        for w in keys.windows(2) {
            let (a, b) = (w[0], w[1]);
            if t <= b.t {
                let span = (b.t - a.t).max(1e-6);
                let u = a.ease.apply((t - a.t) / span);
                return a.v + (b.v - a.v) * u;
            }
        }
        keys[keys.len() - 1].v
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Clip {
    pub name: String,
    /// Seconds.
    pub duration: f32,
    #[serde(default)]
    pub looping: bool,
    #[serde(default)]
    pub description: String,
    pub tracks: Vec<Track>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipLibrary {
    pub format: String,
    pub clips: Vec<Clip>,
}

/// A track resolved against a skeleton.
#[derive(Debug, Clone)]
pub(crate) enum Binding {
    Bone(usize),
    Part(String),
}

#[derive(Debug, Clone)]
pub struct BoundClip {
    pub clip: Clip,
    pub(crate) bindings: Vec<Binding>,
}

impl ClipLibrary {
    pub fn from_json(text: &str) -> Result<Self, RigError> {
        let lib: ClipLibrary = serde_json::from_str(text).map_err(|e| RigError::Json(e.to_string()))?;
        if lib.format != "mascot-clips/0.2" {
            return Err(RigError::Format(format!("unexpected clip format '{}'", lib.format)));
        }
        Ok(lib)
    }

    pub fn bind(&self, skeleton: &Skeleton, parts: &[String]) -> Result<Vec<BoundClip>, RigError> {
        self.clips.iter().map(|c| bind_clip(c, skeleton, parts)).collect()
    }
}

pub fn bind_clip(clip: &Clip, skeleton: &Skeleton, parts: &[String]) -> Result<BoundClip, RigError> {
    if clip.duration <= 0.0 {
        return Err(RigError::Invalid(format!("clip {} has no duration", clip.name)));
    }
    let mut bindings = Vec::new();
    for tr in &clip.tracks {
        if tr.keys.is_empty() {
            return Err(RigError::Invalid(format!("clip {} track {} has no keys", clip.name, tr.target)));
        }
        if tr.keys.windows(2).any(|w| w[1].t < w[0].t) || tr.keys.iter().any(|k| k.t < 0.0 || k.t > clip.duration + 1e-4) {
            return Err(RigError::Invalid(format!("clip {} track {} keys unsorted/out of range", clip.name, tr.target)));
        }
        if !clip.looping {
            let n = tr.property.neutral();
            let (a, b) = (tr.keys[0].v, tr.keys[tr.keys.len() - 1].v);
            if (a - n).abs() > 1e-4 || (b - n).abs() > 1e-4 {
                return Err(RigError::Invalid(format!(
                    "clip {} track {} must start and end at rest ({n})",
                    clip.name, tr.target
                )));
            }
        }
        bindings.push(match tr.property {
            Property::Opacity => {
                if !parts.iter().any(|p| p == &tr.target) {
                    return Err(RigError::Invalid(format!("clip {}: unknown part {}", clip.name, tr.target)));
                }
                Binding::Part(tr.target.clone())
            }
            _ => Binding::Bone(
                skeleton
                    .find(&tr.target)
                    .ok_or_else(|| RigError::UnknownBone { context: format!("clip {}", clip.name), bone: tr.target.clone() })?,
            ),
        });
    }
    Ok(BoundClip { clip: clip.clone(), bindings })
}

impl BoundClip {
    /// Adds this clip's contribution at local time `t` into `pose`.
    pub fn apply(&self, t: f32, weight: f32, pose: &mut Pose) {
        for (tr, b) in self.clip.tracks.iter().zip(&self.bindings) {
            let v = tr.sample(t);
            match (b, tr.property) {
                (Binding::Bone(i), Property::X) => pose.bones[*i].dx += v * weight,
                (Binding::Bone(i), Property::Y) => pose.bones[*i].dy += v * weight,
                (Binding::Bone(i), Property::Rotation) => pose.bones[*i].rotation_deg += v * weight,
                (Binding::Bone(i), Property::ScaleX) => pose.bones[*i].scale_x *= 1.0 + (v - 1.0) * weight,
                (Binding::Bone(i), Property::ScaleY) => pose.bones[*i].scale_y *= 1.0 + (v - 1.0) * weight,
                (Binding::Part(p), Property::Opacity) => {
                    *pose.part_opacity.entry(p.clone()).or_insert(1.0) *= 1.0 + (v - 1.0) * weight;
                }
                _ => {}
            }
        }
    }

    /// Largest |rotation| offset per bone reached by this clip (sampled).
    pub fn max_rotation(&self, bone: usize) -> f32 {
        let mut m: f32 = 0.0;
        for (tr, b) in self.clip.tracks.iter().zip(&self.bindings) {
            if matches!(b, Binding::Bone(i) if *i == bone) && tr.property == Property::Rotation {
                for s in 0..=200 {
                    m = m.max(tr.sample(self.clip.duration * s as f32 / 200.0).abs());
                }
            }
        }
        m
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eases_are_anchored() {
        for e in [Ease::Linear, Ease::InQuad, Ease::OutQuad, Ease::InOutQuad, Ease::InOutCubic, Ease::InOutSine, Ease::OutBack] {
            assert!(e.apply(0.0).abs() < 1e-6, "{e:?}");
            assert!((e.apply(1.0) - 1.0).abs() < 1e-6, "{e:?}");
        }
        assert_eq!(Ease::Step.apply(0.99), 0.0);
    }

    #[test]
    fn track_interpolates_and_clamps() {
        let tr = Track {
            target: "head".into(),
            property: Property::Rotation,
            keys: vec![
                Key { t: 0.0, v: 0.0, ease: Ease::Linear },
                Key { t: 1.0, v: 10.0, ease: Ease::Linear },
                Key { t: 2.0, v: 0.0, ease: Ease::Linear },
            ],
        };
        assert_eq!(tr.sample(-1.0), 0.0);
        assert!((tr.sample(0.5) - 5.0).abs() < 1e-5);
        assert!((tr.sample(1.5) - 5.0).abs() < 1e-5);
        assert_eq!(tr.sample(9.0), 0.0);
    }
}
