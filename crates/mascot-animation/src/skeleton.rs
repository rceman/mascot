//! Bone hierarchy, poses and world-transform evaluation.

use crate::math::Affine;
use crate::rig::{RigError, RigFile, TransformDef};
use std::collections::HashMap;

/// Local bone transform. `x`/`y` are parent-local pixels, rotation in degrees
/// (positive = clockwise on screen).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    pub x: f32,
    pub y: f32,
    pub rotation_deg: f32,
    pub scale_x: f32,
    pub scale_y: f32,
}

impl Transform {
    pub const IDENTITY: Transform = Transform { x: 0.0, y: 0.0, rotation_deg: 0.0, scale_x: 1.0, scale_y: 1.0 };

    /// `T(x, y) * R(rotation) * S(scale_x, scale_y)`
    pub fn to_affine(&self) -> Affine {
        Affine::translate(self.x, self.y)
            .mul(Affine::rotate_deg(self.rotation_deg))
            .mul(Affine::scale(self.scale_x, self.scale_y))
    }
}

impl From<TransformDef> for Transform {
    fn from(t: TransformDef) -> Self {
        Transform { x: t.x, y: t.y, rotation_deg: t.rotation_deg, scale_x: t.scale_x, scale_y: t.scale_y }
    }
}

/// Animated offset applied on top of a bone's rest transform.
/// Translation and rotation add; scale multiplies.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoneOffset {
    pub dx: f32,
    pub dy: f32,
    pub rotation_deg: f32,
    pub scale_x: f32,
    pub scale_y: f32,
}

impl Default for BoneOffset {
    fn default() -> Self {
        BoneOffset { dx: 0.0, dy: 0.0, rotation_deg: 0.0, scale_x: 1.0, scale_y: 1.0 }
    }
}

impl BoneOffset {
    pub fn is_identity(&self) -> bool {
        *self == BoneOffset::default()
    }
}

#[derive(Debug, Clone)]
pub struct Bone {
    pub id: String,
    pub parent: Option<usize>,
    pub rest: Transform,
    pub safe_rotation_deg: [f32; 2],
}

/// Whole-character placement applied above the root bone.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RootPlacement {
    /// Horizontal mirror about the root bone's pivot (flips the whole rig).
    pub mirror: bool,
}

impl Default for RootPlacement {
    fn default() -> Self {
        RootPlacement { mirror: false }
    }
}

/// Per-frame animated state: one offset per bone, one opacity multiplier per part.
#[derive(Debug, Clone, PartialEq)]
pub struct Pose {
    pub bones: Vec<BoneOffset>,
    pub part_opacity: HashMap<String, f32>,
}

impl Pose {
    pub fn is_rest(&self) -> bool {
        self.bones.iter().all(BoneOffset::is_identity) && self.part_opacity.values().all(|&o| o == 1.0)
    }
    pub fn reset(&mut self) {
        self.bones.iter_mut().for_each(|b| *b = BoneOffset::default());
        self.part_opacity.clear();
    }
}

#[derive(Debug, Clone)]
pub struct Skeleton {
    /// Topologically ordered: parents always precede children.
    pub bones: Vec<Bone>,
    index: HashMap<String, usize>,
}

impl Skeleton {
    pub fn from_rig(rig: &RigFile) -> Result<Self, RigError> {
        rig.validate()?;
        // Order parents before children (stable with respect to file order).
        let mut order: Vec<usize> = Vec::with_capacity(rig.bones.len());
        let mut placed: HashMap<&str, usize> = HashMap::new();
        while order.len() < rig.bones.len() {
            let before = order.len();
            for (i, b) in rig.bones.iter().enumerate() {
                if placed.contains_key(b.id.as_str()) {
                    continue;
                }
                let ready = match &b.parent {
                    None => true,
                    Some(p) => placed.contains_key(p.as_str()),
                };
                if ready {
                    placed.insert(&b.id, order.len());
                    order.push(i);
                }
            }
            if order.len() == before {
                return Err(RigError::BoneCycle("unresolvable hierarchy".into()));
            }
        }
        let bones: Vec<Bone> = order
            .iter()
            .map(|&i| {
                let b = &rig.bones[i];
                Bone {
                    id: b.id.clone(),
                    parent: b.parent.as_ref().map(|p| placed[p.as_str()]),
                    rest: b.rest.into(),
                    safe_rotation_deg: b.safe_rotation_deg,
                }
            })
            .collect();
        let index = bones.iter().enumerate().map(|(i, b)| (b.id.clone(), i)).collect();
        Ok(Skeleton { bones, index })
    }

    pub fn find(&self, id: &str) -> Option<usize> {
        self.index.get(id).copied()
    }

    pub fn rest_pose(&self) -> Pose {
        Pose { bones: vec![BoneOffset::default(); self.bones.len()], part_opacity: HashMap::new() }
    }

    /// Local transform of bone `i` under `pose`.
    pub fn local(&self, i: usize, pose: &Pose) -> Transform {
        let r = self.bones[i].rest;
        let o = pose.bones[i];
        Transform {
            x: r.x + o.dx,
            y: r.y + o.dy,
            rotation_deg: r.rotation_deg + o.rotation_deg,
            scale_x: r.scale_x * o.scale_x,
            scale_y: r.scale_y * o.scale_y,
        }
    }

    /// World (canvas-space) transform of every bone, indexed like `self.bones`.
    pub fn world(&self, pose: &Pose, placement: RootPlacement) -> Vec<Affine> {
        let mut out: Vec<Affine> = Vec::with_capacity(self.bones.len());
        for (i, b) in self.bones.iter().enumerate() {
            let local = self.local(i, pose).to_affine();
            let w = match b.parent {
                Some(p) => out[p].mul(local),
                None => {
                    // Mirror about the root pivot: T(pivot) * S(-1, 1) * T(-pivot) * local
                    // == T(x, y) * S(-1, 1) * R * S for a root whose origin is the pivot.
                    if placement.mirror {
                        let l = self.local(i, pose);
                        Affine::translate(l.x, l.y)
                            .mul(Affine::scale(-1.0, 1.0))
                            .mul(Affine::rotate_deg(l.rotation_deg))
                            .mul(Affine::scale(l.scale_x, l.scale_y))
                    } else {
                        local
                    }
                }
            };
            out.push(w);
        }
        out
    }

    /// Rest-pose world transforms (bind pose), used for mesh skinning.
    pub fn bind_world(&self) -> Vec<Affine> {
        self.world(&self.rest_pose(), RootPlacement::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::tiny_rig;

    #[test]
    fn world_pivots_match_rest_canvas_positions() {
        let rig = tiny_rig();
        let sk = Skeleton::from_rig(&rig).unwrap();
        let w = sk.world(&sk.rest_pose(), RootPlacement::default());
        let head = sk.find("head").unwrap();
        assert_eq!(w[head].apply([0.0, 0.0]), [120.0, 40.0]);
    }

    #[test]
    fn child_follows_parent_rotation_about_parent_pivot() {
        let rig = tiny_rig();
        let sk = Skeleton::from_rig(&rig).unwrap();
        let mut pose = sk.rest_pose();
        let neck = sk.find("neck").unwrap();
        let head = sk.find("head").unwrap();
        pose.bones[neck].rotation_deg = 90.0;
        let w = sk.world(&pose, RootPlacement::default());
        // neck pivot at (100, 60); head is +20,-20 from it -> rotated 90deg cw -> +20,+20
        let p = w[head].apply([0.0, 0.0]);
        assert!((p[0] - 120.0).abs() < 1e-4 && (p[1] - 80.0).abs() < 1e-4, "{p:?}");
    }

    #[test]
    fn mirror_reflects_about_root_pivot() {
        let rig = tiny_rig();
        let sk = Skeleton::from_rig(&rig).unwrap();
        let w = sk.world(&sk.rest_pose(), RootPlacement { mirror: true });
        let head = sk.find("head").unwrap();
        // root pivot x = 100 -> head x 120 mirrors to 80, y unchanged
        assert_eq!(w[head].apply([0.0, 0.0]), [80.0, 40.0]);
        assert!(w[head].det() < 0.0, "mirrored transforms must flip handedness");
    }

    #[test]
    fn topological_order_is_enforced_for_out_of_order_files() {
        let mut rig = tiny_rig();
        rig.bones.reverse();
        let sk = Skeleton::from_rig(&rig).unwrap();
        for (i, b) in sk.bones.iter().enumerate() {
            if let Some(p) = b.parent {
                assert!(p < i);
            }
        }
    }
}
