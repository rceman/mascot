//! RigProfile schema (`mascot-rig-profile/1`) and `profile check`.

use serde::Deserialize;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Deserialize)]
pub enum LmClass {
    R,
    J,
    F,
    T,
    S,
}

/// `[class, x, y, r]`
#[derive(Clone, Copy, Debug)]
pub struct LandmarkDef {
    pub class: LmClass,
    pub x: f64,
    pub y: f64,
    pub r: f64,
}

impl<'de> Deserialize<'de> for LandmarkDef {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let (class, x, y, r) = <(LmClass, f64, f64, f64)>::deserialize(d)?;
        Ok(LandmarkDef { class, x, y, r })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewDef {
    pub facing: String,
    pub near_side: String,
    pub far_side: String,
    pub rotation: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FgDef {
    pub alpha_min: u8,
    pub bg_delta: u8,
    pub bg_uniform: u8,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanvasDef {
    pub aspect: [u32; 2],
    pub min_px: u32,
    pub safe_margin: f64,
    pub crop_margin: f64,
    pub foreground: FgDef,
    pub on_character: BTreeMap<String, f64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroundDef {
    pub near: f64,
    pub far: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SocketDef {
    pub parent: String,
    pub accepts: Vec<String>,
}

/// Pose constraint: exactly one kind key plus its parameters.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PoseConstraint {
    #[serde(default)]
    pub angle: Option<Vec<String>>,
    #[serde(default)]
    pub bend: Option<Vec<String>>,
    #[serde(default)]
    pub order_x: Option<Vec<String>>,
    #[serde(default)]
    pub on_ground: Option<Vec<String>>,
    #[serde(default)]
    pub no_cross: Option<Vec<String>>,
    #[serde(default)]
    pub tol: Option<f64>,
    #[serde(default)]
    pub range: Option<[f64; 2]>,
    #[serde(default)]
    pub min_gap: Option<f64>,
    #[serde(default)]
    pub line: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProportionDef {
    pub name: String,
    #[serde(default)]
    pub dy: Option<Vec<String>>,
    #[serde(default)]
    pub chain: Option<String>,
    pub range: [f64; 2],
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClearanceDef {
    pub name: String,
    pub at: String,
    pub toward: String,
    pub fail: f64,
    pub warn: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SizesDef {
    pub generation: u32,
    pub annotated: u32,
}

/// Guide shape primitive: one kind key plus params.
#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct ShapeDef {
    #[serde(default)]
    pub capsule: Option<Vec<serde_json::Value>>,
    #[serde(default)]
    pub circle: Option<serde_json::Value>,
    #[serde(default)]
    pub ellipse: Option<serde_json::Value>,
    #[serde(default)]
    pub poly: Option<Vec<serde_json::Value>>,
    #[serde(default)]
    pub rrect: Option<serde_json::Value>,
    #[serde(default)]
    pub w: Option<f64>,
    #[serde(default)]
    pub r: Option<f64>,
    #[serde(default)]
    pub rx: Option<f64>,
    #[serde(default)]
    pub ry: Option<f64>,
    #[serde(default)]
    pub h: Option<f64>,
    #[serde(default)]
    pub offset: Option<[f64; 2]>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LayerDef {
    pub name: String,
    pub fill: String,
    #[serde(default)]
    pub line: Option<String>,
    #[serde(default)]
    pub line_w: Option<f64>,
    pub shapes: Vec<ShapeDef>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GuideDef {
    pub sizes: SizesDef,
    pub background: String,
    pub line: String,
    pub line_w: f64,
    pub layers: Vec<LayerDef>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub format: String,
    pub id: String,
    pub revision: u32,
    #[serde(default)]
    pub status: Option<String>,
    pub view: ViewDef,
    pub canvas: CanvasDef,
    pub height_ref: [String; 2],
    pub ground: GroundDef,
    pub roles: BTreeMap<String, Option<String>>,
    pub landmarks: BTreeMap<String, LandmarkDef>,
    #[serde(default)]
    pub appendage_types: Vec<String>,
    #[serde(default)]
    pub sockets: BTreeMap<String, SocketDef>,
    #[serde(default)]
    pub zones: BTreeMap<String, [f64; 4]>,
    #[serde(default)]
    pub chains: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub pose: Vec<PoseConstraint>,
    #[serde(default)]
    pub proportions: Vec<ProportionDef>,
    #[serde(default)]
    pub clearance: Vec<ClearanceDef>,
    pub guide: GuideDef,
}

impl Profile {
    pub fn load(path: &Path) -> Result<(Self, Vec<u8>), String> {
        let bytes = std::fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
        let p: Profile =
            serde_json::from_slice(&bytes).map_err(|e| format!("parse {}: {e}", path.display()))?;
        if p.format != "mascot-rig-profile/1" {
            return Err(format!(
                "{}: format must be mascot-rig-profile/1",
                path.display()
            ));
        }
        Ok((p, bytes))
    }

    /// Load + semantic check: usable for validation only when clean.
    pub fn load_checked(path: &Path) -> Result<(Self, Vec<u8>), String> {
        let (p, bytes) = Self::load(path)?;
        let errs = check_profile(&p);
        if !errs.is_empty() {
            return Err(format!(
                "{}: invalid profile: {}",
                path.display(),
                errs.join("; ")
            ));
        }
        Ok((p, bytes))
    }

    pub fn landmark(&self, id: &str) -> Option<[f64; 2]> {
        self.landmarks.get(id).map(|l| [l.x, l.y])
    }

    /// Parent chain of `role` (excluding itself) up to the root.
    pub fn ancestors(&self, role: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut cur = self.roles.get(role).cloned().flatten();
        while let Some(r) = cur {
            out.push(r.clone());
            cur = self.roles.get(&r).cloned().flatten();
        }
        out
    }
}

/// Semantic profile validation; returns human-readable error strings.
pub fn check_profile(p: &Profile) -> Vec<String> {
    let mut errs = Vec::new();
    let known_lm = |errs: &mut Vec<String>, ctx: &str, id: &str| {
        if !p.landmarks.contains_key(id) {
            errs.push(format!("{ctx}: unknown landmark '{id}'"));
        }
    };

    // roles: parents exist, acyclic, single root
    let mut roots = 0;
    for (r, par) in &p.roles {
        match par {
            None => roots += 1,
            Some(par) => {
                if !p.roles.contains_key(par) {
                    errs.push(format!("role '{r}': unknown parent '{par}'"));
                }
                let mut seen = HashSet::new();
                let mut cur = Some(par.clone());
                while let Some(c) = cur {
                    if c == *r {
                        errs.push(format!("role '{r}': cycle via '{c}'"));
                        break;
                    }
                    if !seen.insert(c.clone()) {
                        errs.push(format!("role '{r}': cycle"));
                        break;
                    }
                    cur = p.roles.get(&c).cloned().flatten();
                }
            }
        }
    }
    if roots != 1 {
        errs.push(format!("roles: expected exactly one root, found {roots}"));
    }
    // every role has a landmark of the same id: J/F (or R for the root role);
    // every J/F landmark is a role
    for (r, par) in &p.roles {
        match p.landmarks.get(r) {
            Some(l) if l.class == LmClass::J || l.class == LmClass::F => {}
            Some(l) if l.class == LmClass::R && par.is_none() => {}
            _ => errs.push(format!("role '{r}': no J/F landmark with the same id")),
        }
    }
    for (id, l) in &p.landmarks {
        if (l.class == LmClass::J || l.class == LmClass::F) && !p.roles.contains_key(id) {
            errs.push(format!(
                "landmark '{id}' (class {:?}) is not a role",
                l.class
            ));
        }
    }
    // sockets
    for (sid, s) in &p.sockets {
        known_lm(&mut errs, "socket", sid);
        if !p.roles.contains_key(&s.parent) {
            errs.push(format!(
                "socket '{sid}': unknown parent role '{}'",
                s.parent
            ));
        }
        for t in &s.accepts {
            if !p.appendage_types.contains(t) {
                errs.push(format!("socket '{sid}': unknown appendage type '{t}'"));
            }
        }
    }
    // chains
    for (cid, chain) in &p.chains {
        for id in chain {
            known_lm(&mut errs, &format!("chain '{cid}'"), id);
        }
    }
    // pose constraints
    for (i, c) in p.pose.iter().enumerate() {
        let kinds = [
            c.angle.is_some(),
            c.bend.is_some(),
            c.order_x.is_some(),
            c.on_ground.is_some(),
            c.no_cross.is_some(),
        ]
        .iter()
        .filter(|b| **b)
        .count();
        if kinds != 1 {
            errs.push(format!(
                "pose[{i}]: exactly one constraint kind required ({kinds} found)"
            ));
        }
        if let Some(v) = &c.angle {
            if v.len() != 2 || c.tol.is_none() {
                errs.push(format!("pose[{i}].angle: [a,b] + tol required"));
            } else {
                for id in v {
                    known_lm(&mut errs, &format!("pose[{i}].angle"), id);
                }
            }
        }
        if let Some(v) = &c.bend {
            if v.len() != 3 || c.range.is_none() {
                errs.push(format!("pose[{i}].bend: [a,b,c] + range required"));
            } else {
                for id in v {
                    known_lm(&mut errs, &format!("pose[{i}].bend"), id);
                }
            }
        }
        if let Some(v) = &c.order_x {
            if v.len() != 2 || c.min_gap.is_none() {
                errs.push(format!("pose[{i}].order_x: [a,b] + min_gap required"));
            } else {
                for id in v {
                    known_lm(&mut errs, &format!("pose[{i}].order_x"), id);
                }
            }
        }
        if let Some(v) = &c.on_ground {
            if v.is_empty() || c.tol.is_none() {
                errs.push(format!("pose[{i}].on_ground: pts + tol required"));
            }
            for id in v {
                known_lm(&mut errs, &format!("pose[{i}].on_ground"), id);
            }
            match c.line.as_deref() {
                Some("near") | Some("far") => {}
                other => errs.push(format!(
                    "pose[{i}].on_ground: line must be near|far, got {other:?}"
                )),
            }
        }
        if let Some(v) = &c.no_cross {
            if v.len() != 2 {
                errs.push(format!("pose[{i}].no_cross: [chainA, chainB] required"));
            } else {
                for id in v {
                    if !p.chains.contains_key(id) {
                        errs.push(format!("pose[{i}].no_cross: unknown chain '{id}'"));
                    }
                }
            }
        }
    }
    // proportions
    for pr in &p.proportions {
        if let Some(dy) = &pr.dy {
            if dy.len() != 2 {
                errs.push(format!("proportion '{}': dy needs [a,b]", pr.name));
            } else {
                for id in dy {
                    known_lm(&mut errs, &format!("proportion '{}'", pr.name), id);
                }
            }
        } else if let Some(ch) = &pr.chain {
            if !p.chains.contains_key(ch) {
                errs.push(format!("proportion '{}': unknown chain '{ch}'", pr.name));
            }
        } else {
            errs.push(format!("proportion '{}': dy or chain required", pr.name));
        }
    }
    // clearance
    for c in &p.clearance {
        known_lm(&mut errs, &format!("clearance '{}'", c.name), &c.at);
        known_lm(&mut errs, &format!("clearance '{}'", c.name), &c.toward);
    }
    // height_ref
    for id in &p.height_ref {
        known_lm(&mut errs, "height_ref", id);
    }
    // guide shape landmark refs
    for layer in &p.guide.layers {
        for sh in &layer.shapes {
            for v in shape_lm_refs(sh) {
                known_lm(&mut errs, &format!("guide layer '{}'", layer.name), &v);
            }
        }
    }
    // zones bounds
    for (zid, z) in &p.zones {
        if !(0.0..=1.0).contains(&z[0])
            || !(0.0..=1.0).contains(&z[2])
            || !(0.0..=1.0).contains(&z[1])
            || !(0.0..=1.0).contains(&z[3])
        {
            errs.push(format!("zone '{zid}': bounds outside [0,1]"));
        }
    }
    // dummy satisfies its own pose/proportion constraints
    if errs.is_empty() {
        errs.extend(dummy_self_check(p));
    }
    errs
}

/// landmark ids referenced by a shape spec (string point args).
pub fn shape_lm_refs(sh: &ShapeDef) -> Vec<String> {
    let mut out = Vec::new();
    let pts: Vec<&serde_json::Value> = [
        sh.capsule.iter().flatten().collect::<Vec<_>>(),
        sh.poly.iter().flatten().collect::<Vec<_>>(),
        sh.circle.iter().collect(),
        sh.ellipse.iter().collect(),
        sh.rrect.iter().collect(),
    ]
    .concat();
    for v in pts {
        if let serde_json::Value::String(s) = v {
            out.push(s.clone());
        }
    }
    out
}

/// Resolve a shape point arg (landmark id or [x,y]) into normalized coords.
pub fn resolve_pt(p: &Profile, v: &serde_json::Value) -> Result<[f64; 2], String> {
    match v {
        serde_json::Value::String(id) => p
            .landmark(id)
            .ok_or_else(|| format!("unknown landmark '{id}'")),
        serde_json::Value::Array(a) if a.len() == 2 => {
            Ok([a[0].as_f64().unwrap_or(0.0), a[1].as_f64().unwrap_or(0.0)])
        }
        other => Err(format!("bad shape point {other}")),
    }
}

/// Evaluate pose + proportion constraints against the profile's own landmark set.
fn dummy_self_check(p: &Profile) -> Vec<String> {
    let lm: HashMap<String, [f64; 2]> = p
        .landmarks
        .iter()
        .map(|(k, l)| (k.clone(), [l.x, l.y]))
        .collect();
    let mut errs = Vec::new();
    crate::validate::eval_pose_constraints(p, &lm, &mut |sev, code, subj, msg| {
        let _ = sev;
        errs.push(format!("dummy pose: {code} {subj}: {msg}"));
    });
    crate::validate::eval_proportions(p, &lm, &mut |sev, code, subj, msg| {
        let _ = sev;
        errs.push(format!("dummy proportions: {code} {subj}: {msg}"));
    });
    errs
}
