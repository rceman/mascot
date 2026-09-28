//! Platform-independent mascot rig runtime (rig v0.2).
//!
//! Pipeline per frame:
//!
//! ```text
//! Player::advance -> Player::evaluate(Pose) -> Skeleton::world -> Rig::draw_list
//!   -> renderer: fills -> silhouette -> runtime outline/shadow -> line art
//! ```
//!
//! Nothing here ticks on its own; hosts drive frames only while
//! [`player::Player::is_static`] is false.

pub mod attachment;
pub mod clip;
pub mod idle;
pub mod math;
pub mod player;
pub mod rig;
pub mod skeleton;

#[cfg(test)]
pub(crate) mod test_support;

pub use attachment::{Attachment, MeshAttachment, Slot, SpriteAttachment};
pub use clip::{BoundClip, Clip, ClipLibrary, Ease, Key, Property, Track};
pub use math::Affine;
pub use player::{Player, PlayerEvent};
pub use rig::{JointGuardDef, RigError, RigFile, Role};
pub use skeleton::{BoneOffset, Pose, RootPlacement, Skeleton, Transform};

use std::path::{Path, PathBuf};

/// A loaded rig: file data + skeleton + draw-ordered slots.
#[derive(Debug, Clone)]
pub struct Rig {
    pub file: RigFile,
    pub skeleton: Skeleton,
    /// Sorted by z (stable), i.e. draw order.
    pub slots: Vec<Slot>,
    pub parts: Vec<String>,
    pub dir: PathBuf,
}

/// One sprite to draw this frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DrawItem {
    pub slot: usize,
    /// Image pixels -> canvas pixels.
    pub transform: Affine,
    pub opacity: f32,
    pub role: Role,
}

impl Rig {
    pub fn from_file(file: RigFile) -> Result<Self, RigError> {
        let skeleton = Skeleton::from_rig(&file)?;
        let mut slots = file
            .attachments
            .iter()
            .map(|a| Slot::from_def(a, &skeleton))
            .collect::<Result<Vec<_>, _>>()?;
        slots.sort_by_key(|s| s.z);
        let mut parts: Vec<String> = Vec::new();
        for s in &slots {
            if !parts.contains(&s.part) {
                parts.push(s.part.clone());
            }
        }
        Ok(Rig { file, skeleton, slots, parts, dir: PathBuf::new() })
    }

    pub fn load(dir: impl AsRef<Path>) -> Result<Self, RigError> {
        let dir = dir.as_ref();
        let text = std::fs::read_to_string(dir.join("rig.json"))
            .map_err(|e| RigError::Json(format!("{}: {e}", dir.join("rig.json").display())))?;
        let mut rig = Rig::from_file(RigFile::from_json(&text)?)?;
        rig.dir = dir.to_path_buf();
        Ok(rig)
    }

    pub fn load_clips(&self) -> Result<Vec<BoundClip>, RigError> {
        let path = self.dir.join("clips.json");
        let text = std::fs::read_to_string(&path).map_err(|e| RigError::Json(format!("{}: {e}", path.display())))?;
        ClipLibrary::from_json(&text)?.bind(&self.skeleton, &self.parts)
    }

    pub fn canvas_size(&self) -> [u32; 2] {
        [self.file.source.width, self.file.source.height]
    }

    pub fn outline_radius(&self) -> f32 {
        self.file.outline.radius_canvas_px
    }

    /// Draw-ordered sprite placements for `world` (from [`Skeleton::world`]).
    pub fn draw_list(&self, world: &[Affine], pose: &Pose) -> Vec<DrawItem> {
        self.slots
            .iter()
            .enumerate()
            .filter(|(_, s)| s.visible)
            .filter_map(|(i, s)| match &s.attachment {
                Attachment::Sprite(sp) => {
                    let opacity = s.opacity * pose.part_opacity.get(&s.part).copied().unwrap_or(1.0);
                    (opacity > 0.0).then(|| DrawItem {
                        slot: i,
                        transform: attachment::sprite_transform(world[s.bone], sp),
                        opacity,
                        role: s.role,
                    })
                }
                // Mesh attachments are skinned by `attachment::skin_mesh`; the v0.2
                // Direct2D renderer does not draw them yet.
                Attachment::Mesh(_) => None,
            })
            .collect()
    }

    /// Clamp helper for interactive tools: limits a rotation offset to the bone's safe range.
    pub fn clamp_to_safe(&self, bone: usize, deg: f32) -> f32 {
        let [lo, hi] = self.skeleton.bones[bone].safe_rotation_deg;
        if lo == 0.0 && hi == 0.0 { deg } else { deg.clamp(lo, hi) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::tiny_rig;

    #[test]
    fn draw_list_is_z_ordered_and_honours_opacity() {
        let rig = Rig::from_file(tiny_rig()).unwrap();
        let mut pose = rig.skeleton.rest_pose();
        let world = rig.skeleton.world(&pose, RootPlacement::default());
        let dl = rig.draw_list(&world, &pose);
        let zs: Vec<i32> = dl.iter().map(|d| rig.slots[d.slot].z).collect();
        assert!(zs.windows(2).all(|w| w[0] <= w[1]));
        pose.part_opacity.insert("head".into(), 0.0);
        let dl2 = rig.draw_list(&world, &pose);
        assert!(dl2.iter().all(|d| rig.slots[d.slot].part != "head"));
    }

    #[test]
    fn rejects_bad_rigs() {
        let mut r = tiny_rig();
        r.bones[1].parent = Some("nope".into());
        assert!(matches!(Rig::from_file(r), Err(RigError::UnknownBone { .. })));
        let mut r = tiny_rig();
        let dup = r.attachments[0].clone();
        r.attachments.push(dup);
        assert!(matches!(Rig::from_file(r), Err(RigError::DuplicateId(_))));
        let mut r = tiny_rig();
        r.bones[0].parent = Some("head".into());
        assert!(Rig::from_file(r).is_err());
    }
}
