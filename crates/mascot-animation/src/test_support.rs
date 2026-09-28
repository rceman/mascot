use crate::rig::RigFile;

/// root(100,100) -> neck(+0,-40 => 100,60) -> head(+20,-20 => 120,40)
pub fn tiny_rig() -> RigFile {
    let json = r#"{
      "format": "mascot-rig", "version": "0.2.0",
      "source": {"asset": "x.png", "width": 200, "height": 200},
      "outline": {"radius_canvas_px": 5.0},
      "bones": [
        {"id": "root", "parent": null, "rest": {"x": 100, "y": 100}},
        {"id": "neck", "parent": "root", "rest": {"x": 0, "y": -40}, "safe_rotation_deg": [-5, 5]},
        {"id": "head", "parent": "neck", "rest": {"x": 20, "y": -20}, "safe_rotation_deg": [-10, 10]}
      ],
      "attachments": [
        {"id": "head.line", "part": "head", "bone": "head", "role": "line", "z": 3,
         "kind": "sprite", "image": "h.line.png", "origin": [-30, -30], "canvas_rect": [90, 10, 60, 60]},
        {"id": "head.fill", "part": "head", "bone": "head", "role": "fill", "z": 2,
         "kind": "sprite", "image": "h.fill.png", "origin": [-30, -30], "canvas_rect": [90, 10, 60, 60]},
        {"id": "body.fill", "part": "body", "bone": "root", "role": "fill", "z": 0,
         "kind": "sprite", "image": "b.fill.png", "origin": [-50, -80], "canvas_rect": [50, 20, 100, 100]}
      ]
    }"#;
    RigFile::from_json(json).unwrap()
}
