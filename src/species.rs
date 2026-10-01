use std::collections::HashMap;
use std::time::Duration;

use ratatui::style::Color;
use ratatui::text::Text;
use serde::Deserialize;

use crate::Result;

const BUILTIN: [&str; 1] = [include_str!("../pets/cat.toml")];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Clip {
    Idle,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Animation {
    frame_ms: u64,
    frames: Vec<String>,
    #[serde(default)]
    sequence: Vec<usize>,
}

impl Animation {
    pub fn frame_at(&self, elapsed: Duration) -> &str {
        let step = (elapsed.as_millis() / u128::from(self.frame_ms)) as usize;
        let index = if self.sequence.is_empty() {
            step % self.frames.len()
        } else {
            self.sequence[step % self.sequence.len()]
        };
        &self.frames[index]
    }

    fn problem(&self) -> Option<&'static str> {
        if self.frame_ms == 0 {
            Some("needs a frame_ms above 0")
        } else if self.frames.is_empty() {
            Some("has no frames")
        } else if self
            .sequence
            .iter()
            .any(|&index| index >= self.frames.len())
        {
            Some("has a sequence pointing past its last frame")
        } else {
            None
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Species {
    pub name: String,
    pub color: Color,
    #[serde(default)]
    pub accents: HashMap<char, Color>,
    clips: HashMap<Clip, Animation>,
}

impl Species {
    pub fn parse(source: &str) -> Result<Species> {
        let species: Species = toml::from_str(source)?;
        if !species.clips.contains_key(&Clip::Idle) {
            return Err(format!("{} has no idle clip", species.name).into());
        }
        for (clip, animation) in &species.clips {
            if let Some(problem) = animation.problem() {
                return Err(format!("{} clip {clip:?} {problem}", species.name).into());
            }
        }
        Ok(species)
    }

    pub fn animation(&self, clip: Clip) -> &Animation {
        self.clips.get(&clip).unwrap_or(&self.clips[&Clip::Idle])
    }

    pub fn size(&self) -> (u16, u16) {
        self.clips
            .values()
            .flat_map(|animation| &animation.frames)
            .map(Text::raw)
            .fold((0, 0), |(width, height), text| {
                (
                    width.max(text.width() as u16),
                    height.max(text.height() as u16),
                )
            })
    }
}

pub fn builtin() -> Vec<Species> {
    BUILTIN
        .iter()
        .map(|source| Species::parse(source).expect("built-in pets are checked by tests"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_builtin_pet_is_valid() {
        for source in BUILTIN {
            if let Err(error) = Species::parse(source) {
                panic!("{error}");
            }
        }
    }

    #[test]
    fn cat_is_measured_by_its_largest_frame() {
        assert_eq!(builtin()[0].size(), (7, 3));
    }

    #[test]
    fn frame_at_follows_the_sequence_and_wraps() {
        let animation = Animation {
            frame_ms: 100,
            frames: vec!["a".into(), "b".into()],
            sequence: vec![0, 0, 1],
        };
        let at = |millis| animation.frame_at(Duration::from_millis(millis));
        assert_eq!([at(0), at(150), at(250), at(300)], ["a", "a", "b", "a"]);
    }
}
