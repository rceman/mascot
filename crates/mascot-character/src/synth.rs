//! Synthetic CharacterSource renderer (`mascot-character-synth/1`).

use crate::draw::{self, Shape};
use crate::png_io::Image;
use crate::profile::ShapeDef;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SynthProfileRef {
    pub id: String,
    pub revision: u32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SynthAnnotation {
    pub status: String,
    pub owner: String,
}

/// Synth spec layer: same shape schema as guide layers, explicit colours.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SynthLayer {
    #[serde(default)]
    pub name: Option<String>,
    pub fill: String,
    #[serde(default)]
    pub line: Option<String>,
    #[serde(default)]
    pub line_w: Option<f64>,
    pub shapes: Vec<ShapeDef>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SynthSpec {
    pub format: String,
    pub character_id: String,
    pub profile: SynthProfileRef,
    pub canvas: [u32; 2],
    pub background: String,
    #[serde(default)]
    pub mirror_x: bool,
    #[serde(default)]
    pub landmarks: BTreeMap<String, [f64; 2]>,
    #[serde(default)]
    pub annotate_exclude: Vec<String>,
    #[serde(default)]
    pub appendages: Vec<[String; 2]>,
    #[serde(default)]
    pub accepted_deviations: Vec<crate::source::AcceptedDeviation>,
    pub annotation: SynthAnnotation,
    pub provenance: crate::source::Provenance,
    #[serde(default)]
    pub generation: Option<serde_json::Value>,
    pub layers: Vec<SynthLayer>,
}

impl SynthSpec {
    pub fn load(path: &Path) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
        let s: SynthSpec =
            serde_json::from_slice(&bytes).map_err(|e| format!("parse {}: {e}", path.display()))?;
        if s.format != "mascot-character-synth/1" {
            return Err(format!(
                "{}: format must be mascot-character-synth/1",
                path.display()
            ));
        }
        Ok(s)
    }

    /// A spec landmark (mirror applied).
    pub fn lm(&self, id: &str) -> Option<[f64; 2]> {
        self.landmarks
            .get(id)
            .map(|&[x, y]| if self.mirror_x { [1.0 - x, y] } else { [x, y] })
    }

    fn pt(&self, v: &serde_json::Value) -> Result<[f64; 2], String> {
        match v {
            serde_json::Value::String(id) => self
                .lm(id)
                .ok_or_else(|| format!("unknown spec landmark '{id}'")),
            serde_json::Value::Array(a) if a.len() == 2 => {
                let x = a[0].as_f64().ok_or("bad point")?;
                let y = a[1].as_f64().ok_or("bad point")?;
                Ok(if self.mirror_x { [1.0 - x, y] } else { [x, y] })
            }
            other => Err(format!("bad shape point {other}")),
        }
    }

    fn shape(&self, def: &ShapeDef) -> Result<Shape, String> {
        if let Some(v) = &def.capsule {
            return Ok(Shape::Capsule {
                a: self.pt(&v[0])?,
                b: self.pt(&v[1])?,
                r: def.w.ok_or("capsule needs w")? / 2.0,
                offset: def
                    .offset
                    .map(|o| if self.mirror_x { [-o[0], o[1]] } else { o })
                    .unwrap_or([0.0, 0.0]),
            });
        }
        if let Some(v) = &def.circle {
            return Ok(Shape::Circle {
                c: self.pt(v)?,
                r: def.r.ok_or("circle needs r")?,
            });
        }
        if let Some(v) = &def.ellipse {
            return Ok(Shape::Ellipse {
                c: self.pt(v)?,
                rx: def.rx.ok_or("ellipse needs rx")?,
                ry: def.ry.ok_or("ellipse needs ry")?,
            });
        }
        if let Some(v) = &def.poly {
            let mut pts = Vec::new();
            for p in v {
                pts.push(self.pt(p)?);
            }
            return Ok(Shape::Poly { pts });
        }
        if let Some(v) = &def.rrect {
            return Ok(Shape::RRect {
                c: self.pt(v)?,
                w: def.w.ok_or("rrect needs w")?,
                h: def.h.ok_or("rrect needs h")?,
                r: def.r.ok_or("rrect needs r")?,
            });
        }
        Err("shape spec has no kind".into())
    }
}

/// Render source.png + source.json for a spec into `out_dir`.
pub fn render_synth(
    spec: &SynthSpec,
    out_dir: &Path,
) -> Result<(Image, serde_json::Value), String> {
    std::fs::create_dir_all(out_dir).map_err(|e| e.to_string())?;
    let bg = if spec.background == "transparent" {
        [0, 0, 0, 0]
    } else {
        crate::guide::hex(&spec.background)?
    };
    let mut img = Image::new(spec.canvas[0], spec.canvas[1], bg);
    for layer in &spec.layers {
        let mut shapes = Vec::new();
        for s in &layer.shapes {
            shapes.push(spec.shape(s)?);
        }
        let fill = crate::guide::hex(&layer.fill)?;
        let line = layer
            .line
            .as_deref()
            .map(crate::guide::hex)
            .transpose()?
            .unwrap_or(fill);
        let lw = layer.line_w.unwrap_or(0.0);
        draw::render_union(&mut img, &shapes, fill, line, lw);
    }
    let png_path = out_dir.join("source.png");
    crate::png_io::save_png(&png_path, &img)?;
    let img_sha = crate::hash::sha256_file(&png_path).map_err(|e| e.to_string())?;
    let lms: serde_json::Map<String, serde_json::Value> = spec
        .landmarks
        .iter()
        .filter(|(k, _)| !spec.annotate_exclude.contains(k))
        .map(|(k, _)| (k.clone(), serde_json::json!(spec.lm(k).unwrap())))
        .collect();
    let manifest = serde_json::json!({
        "format": "mascot-character-source/1",
        "character_id": spec.character_id,
        "profile": {"id": spec.profile.id, "revision": spec.profile.revision},
        "image": {"path": "source.png", "sha256": img_sha},
        "provenance": spec.provenance,
        "generation": spec.generation,
        "annotation": {
            "status": spec.annotation.status,
            "owner": spec.annotation.owner,
            "image_sha256": img_sha,
            "landmarks": lms,
            "appendages": spec.appendages,
            "accepted_deviations": spec.accepted_deviations,
        }
    });
    std::fs::write(
        out_dir.join("source.json"),
        serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())? + "\n",
    )
    .map_err(|e| e.to_string())?;
    Ok((img, manifest))
}
