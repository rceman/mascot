//! Validates the shipped rig v0.2 assets (rig.json + clips.json) against the runtime.

use mascot_animation::{Player, Rig, RootPlacement};
use std::path::PathBuf;

const REQUIRED: &[&str] = &[
    "blink", "double_blink", "look_left", "look_right", "small_head_tilt", "ear_twitch", "tail_flick", "posture_adjust", "stretch",
];

fn rig() -> Rig {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/mascot/rig-v0.2");
    Rig::load(&dir).expect("load shipped rig")
}

#[test]
fn shipped_rig_has_required_hierarchy() {
    let rig = rig();
    let sk = &rig.skeleton;
    let parent = |b: &str| sk.bones[sk.find(b).unwrap()].parent.map(|p| sk.bones[p].id.clone());
    for (child, par) in [
        ("hips", "root"), ("head", "neck"), ("ear_near", "head"), ("ear_far", "head"), ("eye_left", "head"),
        ("eye_right", "head"), ("tail", "hips"), ("leg_near_upper", "hips"), ("leg_near_lower", "leg_near_upper"),
        ("foot_near", "leg_near_lower"), ("leg_far_upper", "hips"), ("leg_far_lower", "leg_far_upper"),
        ("foot_far", "leg_far_lower"), ("arm_near_lower", "arm_near_upper"), ("paw_near", "arm_near_lower"),
        ("arm_far_lower", "arm_far_upper"), ("paw_far", "arm_far_lower"),
    ] {
        assert_eq!(parent(child).as_deref(), Some(par), "{child} parent");
    }
    assert!(rig.outline_radius() > 20.0 && rig.outline_radius() < 50.0);
    assert!(!rig.file.joint_guards.is_empty());
}

#[test]
fn shipped_sprites_exist_with_declared_sizes() {
    let rig = rig();
    for slot in &rig.slots {
        if let mascot_animation::Attachment::Sprite(s) = &slot.attachment {
            let path = rig.dir.join(&s.image);
            let f = std::fs::File::open(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            let dec = png::Decoder::new(std::io::BufReader::new(f));
            let info = dec.read_info().unwrap();
            assert_eq!([info.info().width, info.info().height], s.size, "{}", s.image);
        }
    }
}

#[test]
fn shipped_clips_bind_and_stay_in_safe_ranges() {
    let rig = rig();
    let clips = rig.load_clips().expect("load clips");
    for name in REQUIRED {
        assert!(clips.iter().any(|c| c.clip.name == *name), "missing required clip {name}");
    }
    for c in &clips {
        assert!(!c.clip.looping, "{} should be one-shot", c.clip.name);
        for (i, b) in rig.skeleton.bones.iter().enumerate() {
            let m = c.max_rotation(i);
            let [lo, hi] = b.safe_rotation_deg;
            assert!(m <= lo.abs().max(hi.abs()) + 1e-3, "clip {} drives {} to {m} deg (safe {lo}..{hi})", c.clip.name, b.id);
        }
    }
}

#[test]
fn every_clip_completes_and_returns_to_rest() {
    let rig = rig();
    let clips = rig.load_clips().unwrap();
    let rest = rig.skeleton.world(&rig.skeleton.rest_pose(), RootPlacement::default());
    for (i, c) in clips.iter().enumerate() {
        let mut p = Player::new();
        p.play(i);
        let mut frames = 0;
        while !p.is_static() {
            p.advance(1.0 / 60.0, &clips);
            frames += 1;
            assert!(frames < 60 * 30, "{} never completes", c.clip.name);
        }
        let expected = (c.clip.duration * 60.0).ceil() as i32;
        assert!((frames - expected).abs() <= 1, "{}: {frames} frames vs {expected}", c.clip.name);
        let mut pose = rig.skeleton.rest_pose();
        p.evaluate(&clips, &mut pose);
        let w = rig.skeleton.world(&pose, RootPlacement::default());
        for (a, b) in w.iter().zip(&rest) {
            assert!(a.approx_eq(*b, 1e-4), "{} did not return to rest", c.clip.name);
        }
    }
}
