//! Runtime attachments.
//!
//! v0.2 renders rigid [`Attachment::Sprite`]s. [`Attachment::Mesh`] is part of the
//! model (and skinned here, with tests) so a weighted mesh renderer can be added
//! later without touching bones, clips or the player.

use crate::math::Affine;
use crate::rig::{AttachmentDef, Geometry, RigError, Role};
use crate::skeleton::Skeleton;

#[derive(Debug, Clone)]
pub struct SpriteAttachment {
    pub image: String,
    /// Top-left of the image in owning-bone local pixels.
    pub origin: [f32; 2],
    pub size: [u32; 2],
}

#[derive(Debug, Clone)]
pub struct MeshAttachment {
    pub image: String,
    /// Bind-pose vertex positions in canvas pixels.
    pub vertices: Vec<[f32; 2]>,
    pub uvs: Vec<[f32; 2]>,
    pub triangles: Vec<[u32; 3]>,
    /// Per vertex: (bone index, weight); weights sum to 1.
    pub weights: Vec<Vec<(usize, f32)>>,
}

#[derive(Debug, Clone)]
pub enum Attachment {
    Sprite(SpriteAttachment),
    Mesh(MeshAttachment),
}

#[derive(Debug, Clone)]
pub struct Slot {
    pub id: String,
    pub part: String,
    pub bone: usize,
    pub role: Role,
    pub z: i32,
    pub visible: bool,
    pub opacity: f32,
    pub reconstructed: bool,
    pub attachment: Attachment,
}

impl Slot {
    pub fn from_def(def: &AttachmentDef, skeleton: &Skeleton) -> Result<Self, RigError> {
        let bone = skeleton
            .find(&def.bone)
            .ok_or_else(|| RigError::UnknownBone { context: def.id.clone(), bone: def.bone.clone() })?;
        let attachment = match &def.geometry {
            Geometry::Sprite(s) => Attachment::Sprite(SpriteAttachment {
                image: s.image.clone(),
                origin: s.origin,
                size: [s.canvas_rect[2].max(0) as u32, s.canvas_rect[3].max(0) as u32],
            }),
            Geometry::Mesh(m) => {
                let mut weights = Vec::with_capacity(m.weights.len());
                for vw in &m.weights {
                    let total: f32 = vw.iter().map(|w| w.1).sum();
                    if total <= 0.0 {
                        return Err(RigError::Invalid(format!("mesh {} has an unweighted vertex", def.id)));
                    }
                    weights.push(
                        vw.iter()
                            .map(|(b, w)| {
                                skeleton
                                    .find(b)
                                    .map(|i| (i, w / total))
                                    .ok_or_else(|| RigError::UnknownBone { context: def.id.clone(), bone: b.clone() })
                            })
                            .collect::<Result<Vec<_>, _>>()?,
                    );
                }
                Attachment::Mesh(MeshAttachment {
                    image: m.image.clone(),
                    vertices: m.vertices.clone(),
                    uvs: m.uvs.clone(),
                    triangles: m.triangles.clone(),
                    weights,
                })
            }
        };
        Ok(Slot {
            id: def.id.clone(),
            part: def.part.clone(),
            bone,
            role: def.role,
            z: def.z,
            visible: def.visible,
            opacity: def.opacity,
            reconstructed: def.reconstructed,
            attachment,
        })
    }
}

/// Image-pixel -> canvas transform for a sprite on a bone with world transform `bone_world`.
pub fn sprite_transform(bone_world: Affine, sprite: &SpriteAttachment) -> Affine {
    bone_world.mul(Affine::translate(sprite.origin[0], sprite.origin[1]))
}

/// Linear blend skinning: `v' = sum_i w_i * world_i * bind_i^-1 * v`.
pub fn skin_mesh(mesh: &MeshAttachment, world: &[Affine], bind_inverse: &[Affine]) -> Vec<[f32; 2]> {
    mesh.vertices
        .iter()
        .zip(&mesh.weights)
        .map(|(v, ws)| {
            let m = ws.iter().fold(Affine { a: 0.0, b: 0.0, c: 0.0, d: 0.0, tx: 0.0, ty: 0.0 }, |acc, &(b, w)| {
                acc.add(world[b].mul(bind_inverse[b]).weighted(w))
            });
            m.apply(*v)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skeleton::RootPlacement;
    use crate::test_support::tiny_rig;

    #[test]
    fn rigid_sprite_rest_placement_matches_canvas_rect() {
        let rig = crate::Rig::from_file(tiny_rig()).unwrap();
        let world = rig.skeleton.world(&rig.skeleton.rest_pose(), RootPlacement::default());
        for slot in &rig.slots {
            if let Attachment::Sprite(s) = &slot.attachment {
                let def = rig.file.attachments.iter().find(|a| a.id == slot.id).unwrap();
                let Geometry::Sprite(g) = &def.geometry else { unreachable!() };
                let p = sprite_transform(world[slot.bone], s).apply([0.0, 0.0]);
                assert_eq!(p, [g.canvas_rect[0] as f32, g.canvas_rect[1] as f32]);
            }
        }
    }

    #[test]
    fn mesh_skinning_is_identity_at_bind_pose_and_follows_bones() {
        let rig = crate::Rig::from_file(tiny_rig()).unwrap();
        let sk = &rig.skeleton;
        let head = sk.find("head").unwrap();
        let neck = sk.find("neck").unwrap();
        let mesh = MeshAttachment {
            image: "x".into(),
            vertices: vec![[120.0, 40.0], [140.0, 40.0], [110.0, 50.0]],
            uvs: vec![[0.0, 0.0]; 3],
            triangles: vec![[0, 1, 2]],
            weights: vec![vec![(head, 1.0)], vec![(head, 1.0)], vec![(head, 0.5), (neck, 0.5)]],
        };
        let bind_inv: Vec<Affine> = sk.bind_world().iter().map(|m| m.inverse().unwrap()).collect();
        let rest = skin_mesh(&mesh, &sk.bind_world(), &bind_inv);
        for (a, b) in rest.iter().zip(&mesh.vertices) {
            assert!((a[0] - b[0]).abs() < 1e-4 && (a[1] - b[1]).abs() < 1e-4);
        }
        let mut pose = sk.rest_pose();
        pose.bones[head].rotation_deg = 90.0;
        let posed = skin_mesh(&mesh, &sk.world(&pose, RootPlacement::default()), &bind_inv);
        // vertex 1 is 20px right of the head pivot -> rotates to 20px below it
        assert!((posed[1][0] - 120.0).abs() < 1e-3 && (posed[1][1] - 60.0).abs() < 1e-3, "{:?}", posed[1]);
        // vertex 2 is half-weighted to the static neck: moves half as far as a rigid head vertex would
        let rigid = Affine::translate(120.0, 40.0).mul(Affine::rotate_deg(90.0)).apply([-10.0, 10.0]);
        let expect = [(rigid[0] + 110.0) / 2.0, (rigid[1] + 50.0) / 2.0];
        assert!((posed[2][0] - expect[0]).abs() < 1e-3 && (posed[2][1] - expect[1]).abs() < 1e-3);
    }
}
