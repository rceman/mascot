//! Clip player with explicit static-state detection.
//!
//! The host asks [`Player::is_static`] after every advance: when it returns true
//! no further frames are needed until the next play/interaction, so the host can
//! block in its message loop (no continuous render loop).

use crate::clip::BoundClip;
use crate::skeleton::Pose;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Playing {
    pub clip: usize,
    pub time: f32,
    pub weight: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PlayerEvent {
    Started(usize),
    Completed(usize),
    Stopped(usize),
}

#[derive(Debug, Default, Clone)]
pub struct Player {
    playing: Vec<Playing>,
}

impl Player {
    pub fn new() -> Self {
        Self::default()
    }

    /// Starts (or restarts) a clip.
    pub fn play(&mut self, clip: usize) -> PlayerEvent {
        self.playing.retain(|p| p.clip != clip);
        self.playing.push(Playing { clip, time: 0.0, weight: 1.0 });
        PlayerEvent::Started(clip)
    }

    pub fn stop(&mut self, clip: usize) -> Option<PlayerEvent> {
        let before = self.playing.len();
        self.playing.retain(|p| p.clip != clip);
        (self.playing.len() != before).then_some(PlayerEvent::Stopped(clip))
    }

    /// Stops everything (the pose returns to rest on the next evaluation).
    pub fn reset(&mut self) -> Vec<PlayerEvent> {
        self.playing.drain(..).map(|p| PlayerEvent::Stopped(p.clip)).collect()
    }

    pub fn is_playing(&self, clip: usize) -> bool {
        self.playing.iter().any(|p| p.clip == clip)
    }

    pub fn active(&self) -> &[Playing] {
        &self.playing
    }

    /// True when nothing is animating: the host should stop producing frames.
    pub fn is_static(&self) -> bool {
        self.playing.is_empty()
    }

    /// Advances all clips by `dt` seconds; completed one-shot clips are removed.
    pub fn advance(&mut self, dt: f32, clips: &[BoundClip]) -> Vec<PlayerEvent> {
        let mut events = Vec::new();
        for p in &mut self.playing {
            p.time += dt.max(0.0);
            let c = &clips[p.clip].clip;
            if c.looping {
                p.time %= c.duration;
            }
        }
        self.playing.retain(|p| {
            let c = &clips[p.clip].clip;
            let done = !c.looping && p.time >= c.duration;
            if done {
                events.push(PlayerEvent::Completed(p.clip));
            }
            !done
        });
        events
    }

    /// Resets `pose` to rest and applies every active clip additively.
    pub fn evaluate(&self, clips: &[BoundClip], pose: &mut Pose) {
        pose.reset();
        for p in &self.playing {
            clips[p.clip].apply(p.time, p.weight, pose);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clip::{Clip, Ease, Key, Property, Track, bind_clip};
    use crate::skeleton::Skeleton;
    use crate::test_support::tiny_rig;

    fn nod() -> Clip {
        Clip {
            name: "nod".into(),
            duration: 1.0,
            looping: false,
            description: String::new(),
            tracks: vec![Track {
                target: "head".into(),
                property: Property::Rotation,
                keys: vec![
                    Key { t: 0.0, v: 0.0, ease: Ease::Linear },
                    Key { t: 0.5, v: 6.0, ease: Ease::Linear },
                    Key { t: 1.0, v: 0.0, ease: Ease::Linear },
                ],
            }],
        }
    }

    #[test]
    fn one_shot_clip_completes_and_player_becomes_static_at_rest() {
        let rig = tiny_rig();
        let sk = Skeleton::from_rig(&rig).unwrap();
        let clips = vec![bind_clip(&nod(), &sk, &[]).unwrap()];
        let mut player = Player::new();
        let mut pose = sk.rest_pose();
        assert!(player.is_static());
        player.play(0);
        assert!(!player.is_static());
        player.advance(0.5, &clips);
        player.evaluate(&clips, &mut pose);
        assert!((pose.bones[sk.find("head").unwrap()].rotation_deg - 6.0).abs() < 1e-5);
        let ev = player.advance(0.6, &clips);
        assert_eq!(ev, vec![PlayerEvent::Completed(0)]);
        assert!(player.is_static());
        player.evaluate(&clips, &mut pose);
        assert!(pose.is_rest());
    }

    #[test]
    fn non_looping_clip_must_return_to_rest() {
        let rig = tiny_rig();
        let sk = Skeleton::from_rig(&rig).unwrap();
        let mut c = nod();
        c.tracks[0].keys[2].v = 3.0;
        assert!(bind_clip(&c, &sk, &[]).is_err());
    }

    #[test]
    fn restart_and_reset() {
        let rig = tiny_rig();
        let sk = Skeleton::from_rig(&rig).unwrap();
        let clips = vec![bind_clip(&nod(), &sk, &[]).unwrap()];
        let mut player = Player::new();
        player.play(0);
        player.advance(0.4, &clips);
        player.play(0);
        assert_eq!(player.active()[0].time, 0.0);
        assert_eq!(player.reset(), vec![PlayerEvent::Stopped(0)]);
        assert!(player.is_static());
    }
}
