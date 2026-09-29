//! CharacterSource manifest (`mascot-character-source/1`).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageMeta {
    pub path: String,
    pub sha256: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct BoardRef {
    pub path: String,
    pub sha256: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct PromptRef {
    pub text: String,
    pub sha256: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Generation {
    pub board: BoardRef,
    pub prompt: PromptRef,
}

#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ProvenanceKind {
    Generated,
    Artist,
    Synthetic,
}

impl ProvenanceKind {
    pub fn is_generated(self) -> bool {
        matches!(self, ProvenanceKind::Generated)
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Provenance {
    pub kind: ProvenanceKind,
    /// YYYY-MM-DD calendar date.
    pub created: String,
    pub tool: String,
    #[serde(default)]
    pub note: Option<String>,
}

impl Provenance {
    /// Valid YYYY-MM-DD calendar date?
    pub fn created_valid(&self) -> bool {
        let (y, m, d) = match self.created.split('-').collect::<Vec<_>>().as_slice() {
            [y, m, d] => (
                y.parse::<u32>().unwrap_or(0),
                m.parse::<u32>().unwrap_or(0),
                d.parse::<u32>().unwrap_or(0),
            ),
            _ => return false,
        };
        if y < 1000 || !(1..=12).contains(&m) {
            return false;
        }
        let dim = [
            31,
            28 + (u32::from(y % 4 == 0 && (y % 100 != 0 || y % 400 == 0))),
            31,
            30,
            31,
            30,
            31,
            31,
            30,
            31,
            30,
            31,
        ];
        d >= 1 && d <= dim[(m - 1) as usize]
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct AcceptedDeviation {
    pub landmark: String,
    pub reason: String,
}

#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AnnotationStatus {
    Draft,
    Reviewed,
    Approved,
}

#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AnnotationOwner {
    User,
    Agent,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Annotation {
    pub status: AnnotationStatus,
    pub owner: AnnotationOwner,
    pub image_sha256: String,
    #[serde(default)]
    pub landmarks: BTreeMap<String, [f64; 2]>,
    #[serde(default)]
    pub appendages: Vec<[String; 2]>,
    #[serde(default)]
    pub accepted_deviations: Vec<AcceptedDeviation>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileRefDecl {
    pub id: String,
    pub revision: u32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CharacterSource {
    pub format: String,
    pub character_id: String,
    pub profile: ProfileRefDecl,
    pub image: ImageMeta,
    pub provenance: Provenance,
    #[serde(default)]
    pub generation: Option<Generation>,
    pub annotation: Annotation,
}

impl CharacterSource {
    pub fn load(path: &Path) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
        let m: CharacterSource =
            serde_json::from_slice(&bytes).map_err(|e| format!("parse {}: {e}", path.display()))?;
        if m.format != "mascot-character-source/1" {
            return Err(format!(
                "{}: format must be mascot-character-source/1",
                path.display()
            ));
        }
        Ok(m)
    }
}
