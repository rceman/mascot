//! `rig.json` data model (format `mascot-rig`, version 0.2.x) and validation.
//!
//! Coordinate contract (also recorded verbatim in every rig.json):
//! - canvas: canonical source pixels, origin top-left, +x right, +y down
//! - bone local: parent-local pixels; bone origin == rotation pivot
//! - rotation: degrees, positive = clockwise on screen
//! - world = parent_world * T(x, y) * R(rotation) * S(scale_x, scale_y)
//! - sprite attachment origin: top-left of the image in owning-bone local pixels

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

pub const FORMAT: &str = "mascot-rig";

#[derive(Debug, Clone, PartialEq)]
pub enum RigError {
    Json(String),
    Format(String),
    UnknownBone { context: String, bone: String },
    DuplicateId(String),
    BoneCycle(String),
    Invalid(String),
}

impl fmt::Display for RigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RigError::Json(e) => write!(f, "rig json: {e}"),
            RigError::Format(e) => write!(f, "rig format: {e}"),
            RigError::UnknownBone { context, bone } => write!(f, "{context}: unknown bone '{bone}'"),
            RigError::DuplicateId(id) => write!(f, "duplicate id '{id}'"),
            RigError::BoneCycle(id) => write!(f, "bone hierarchy cycle at '{id}'"),
            RigError::Invalid(e) => write!(f, "invalid rig: {e}"),
        }
    }
}

impl std::error::Error for RigError {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceDef {
    pub asset: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct TransformDef {
    pub x: f32,
    pub y: f32,
    #[serde(default)]
    pub rotation_deg: f32,
    #[serde(default = "one")]
    pub scale_x: f32,
    #[serde(default = "one")]
    pub scale_y: f32,
}

fn one() -> f32 {
    1.0
}
fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoneDef {
    pub id: String,
    pub parent: Option<String>,
    pub rest: TransformDef,
    /// Informational: the bone pivot in canvas pixels at rest.
    #[serde(default)]
    pub rest_world_pivot: Option<[f32; 2]>,
    /// Declared safe rotation range (degrees) relative to rest.
    #[serde(default)]
    pub safe_rotation_deg: [f32; 2],
    #[serde(default)]
    pub mirror: Option<String>,
    #[serde(default)]
    pub note: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// Coloured fill. Contributes to the silhouette used for the runtime outline.
    Fill,
    /// Internal line art. Drawn in z order but excluded from the outline mask.
    Line,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpriteGeometry {
    pub image: String,
    /// Top-left of the image in owning-bone local pixels.
    pub origin: [f32; 2],
    /// Rest placement in canvas pixels: x, y, width, height.
    pub canvas_rect: [i32; 4],
}

/// Weighted mesh attachment (schema-ready; not rendered by the v0.2 renderer).
///
/// Vertices are given in canvas pixels at the bind (rest) pose; each vertex has
/// up to four (bone, weight) influences. See [`crate::attachment::skin_mesh`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshGeometry {
    pub image: String,
    pub vertices: Vec<[f32; 2]>,
    pub uvs: Vec<[f32; 2]>,
    pub triangles: Vec<[u32; 3]>,
    pub weights: Vec<Vec<(String, f32)>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Geometry {
    Sprite(SpriteGeometry),
    Mesh(MeshGeometry),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttachmentDef {
    pub id: String,
    /// Logical part (fill + line attachments of one part share it).
    pub part: String,
    pub bone: String,
    pub role: Role,
    pub z: i32,
    #[serde(default = "yes")]
    pub visible: bool,
    #[serde(default = "one")]
    pub opacity: f32,
    #[serde(default)]
    pub reconstructed: bool,
    #[serde(flatten)]
    pub geometry: Geometry,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutlineDef {
    /// External outline thickness in canvas pixels.
    pub radius_canvas_px: f32,
    #[serde(default)]
    pub model: String,
}

/// A region (circle in the owning bone's local space) expected to stay covered.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JointGuardDef {
    pub id: String,
    pub bone: String,
    /// Centre in the bone's local pixels.
    pub center: [f32; 2],
    pub radius: f32,
    /// Bones whose rotation this guard validates, with the tested range.
    pub drives: Vec<(String, [f32; 2])>,
    /// Minimum coverage (fraction of guard pixels with final alpha >= 0.5).
    #[serde(default = "one")]
    pub min_coverage: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RigFile {
    pub format: String,
    pub version: String,
    pub source: SourceDef,
    #[serde(default)]
    pub coordinate_system: serde_json::Value,
    pub outline: OutlineDef,
    pub bones: Vec<BoneDef>,
    pub attachments: Vec<AttachmentDef>,
    #[serde(default)]
    pub joint_guards: Vec<JointGuardDef>,
}

impl RigFile {
    pub fn from_json(text: &str) -> Result<Self, RigError> {
        let rig: RigFile = serde_json::from_str(text).map_err(|e| RigError::Json(e.to_string()))?;
        if rig.format != FORMAT {
            return Err(RigError::Format(format!("expected format '{FORMAT}', got '{}'", rig.format)));
        }
        if !rig.version.starts_with("0.2.") {
            return Err(RigError::Format(format!("unsupported version {}", rig.version)));
        }
        rig.validate()?;
        Ok(rig)
    }

    pub fn validate(&self) -> Result<(), RigError> {
        let mut bones = HashMap::new();
        for (i, b) in self.bones.iter().enumerate() {
            if bones.insert(b.id.as_str(), i).is_some() {
                return Err(RigError::DuplicateId(b.id.clone()));
            }
        }
        let mut roots = 0;
        for b in &self.bones {
            match &b.parent {
                None => roots += 1,
                Some(p) if !bones.contains_key(p.as_str()) => {
                    return Err(RigError::UnknownBone { context: format!("bone {}", b.id), bone: p.clone() });
                }
                _ => {}
            }
            // walk to the root to detect cycles
            let mut cur = b.parent.as_deref();
            let mut steps = 0;
            while let Some(p) = cur {
                steps += 1;
                if steps > self.bones.len() {
                    return Err(RigError::BoneCycle(b.id.clone()));
                }
                cur = self.bones[bones[p]].parent.as_deref();
            }
            if b.safe_rotation_deg[0] > b.safe_rotation_deg[1] {
                return Err(RigError::Invalid(format!("bone {} safe range is inverted", b.id)));
            }
        }
        if roots != 1 {
            return Err(RigError::Invalid(format!("expected exactly one root bone, found {roots}")));
        }
        let mut ids = HashMap::new();
        for a in &self.attachments {
            if ids.insert(a.id.as_str(), ()).is_some() {
                return Err(RigError::DuplicateId(a.id.clone()));
            }
            if !bones.contains_key(a.bone.as_str()) {
                return Err(RigError::UnknownBone { context: format!("attachment {}", a.id), bone: a.bone.clone() });
            }
            if let Geometry::Mesh(m) = &a.geometry {
                if m.vertices.len() != m.uvs.len() || m.vertices.len() != m.weights.len() {
                    return Err(RigError::Invalid(format!("mesh {} vertex/uv/weight count mismatch", a.id)));
                }
                for w in m.weights.iter().flatten() {
                    if !bones.contains_key(w.0.as_str()) {
                        return Err(RigError::UnknownBone { context: format!("mesh {}", a.id), bone: w.0.clone() });
                    }
                }
            }
        }
        for g in &self.joint_guards {
            if !bones.contains_key(g.bone.as_str()) {
                return Err(RigError::UnknownBone { context: format!("guard {}", g.id), bone: g.bone.clone() });
            }
            for (b, _) in &g.drives {
                if !bones.contains_key(b.as_str()) {
                    return Err(RigError::UnknownBone { context: format!("guard {}", g.id), bone: b.clone() });
                }
            }
        }
        if self.outline.radius_canvas_px <= 0.0 {
            return Err(RigError::Invalid("outline radius must be positive".into()));
        }
        Ok(())
    }
}
