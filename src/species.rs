use std::collections::HashMap;
use std::time::Duration;

use ratatui::style::Color;
use ratatui::text::{Line, Span, Text};
use serde::Deserialize;

use crate::Result;

include!(concat!(env!("OUT_DIR"), "/pets.rs"));

const ESCAPE: char = '^';

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Clip {
    Idle,
    Happy,
    Sad,
    Eat,
    Pet,
    Play,
    Sleep,
    Train,
    Cheer,
}

impl Clip {
    fn fallback(self) -> Option<Clip> {
        match self {
            Clip::Train => Some(Clip::Play),
            Clip::Cheer => Some(Clip::Happy),
            Clip::Idle | Clip::Happy | Clip::Sad | Clip::Eat | Clip::Pet | Clip::Play | Clip::Sleep => None,
        }
    }
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

    pub fn duration(&self) -> Duration {
        let steps = if self.sequence.is_empty() {
            self.frames.len()
        } else {
            self.sequence.len()
        };
        Duration::from_millis(self.frame_ms) * steps as u32
    }

    fn problem(&self) -> Option<&'static str> {
        if self.frame_ms == 0 {
            Some("needs a frame_ms above 0")
        } else if self.frames.is_empty() {
            Some("has no frames")
        } else if self.sequence.iter().any(|&index| index >= self.frames.len()) {
            Some("has a sequence pointing past its last frame")
        } else {
            None
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Species {
    #[serde(rename = "species")]
    pub name: String,
    pub color: Color,
    #[serde(default)]
    colors: HashMap<char, Color>,
    clips: HashMap<Clip, Animation>,
}

impl Species {
    pub fn parse(source: &str) -> Result<Species> {
        let species: Species = toml::from_str(source)?;
        let name = &species.name;
        if !species.clips.contains_key(&Clip::Idle) {
            return Err(format!("{name} has no idle clip").into());
        }
        if let Some(code) = species.colors.keys().find(|code| !matches!(code, '1'..='9')) {
            return Err(format!("{name} defines color {code:?}, only 1 to 9 are allowed").into());
        }
        for (clip, animation) in &species.clips {
            if let Some(problem) = animation.problem() {
                return Err(format!("{name} clip {clip:?} {problem}").into());
            }
            for line in animation.frames.iter().flat_map(|frame| frame.lines()) {
                species
                    .paint_line(line)
                    .map_err(|problem| format!("{name} clip {clip:?} {problem}"))?;
            }
        }
        Ok(species)
    }

    pub fn animation(&self, clip: Clip) -> &Animation {
        self.clips
            .get(&clip)
            .or_else(|| clip.fallback().and_then(|fallback| self.clips.get(&fallback)))
            .unwrap_or(&self.clips[&Clip::Idle])
    }

    pub fn paint(&self, art: &str) -> Text<'static> {
        art.lines()
            .map(|line| self.paint_line(line).expect("frames are checked on parse"))
            .collect()
    }

    fn paint_line(&self, line: &str) -> std::result::Result<Line<'static>, String> {
        let mut color = self.color;
        let mut spans = Vec::new();
        let mut characters = line.chars();
        while let Some(character) = characters.next() {
            if character != ESCAPE {
                spans.push(Span::styled(character.to_string(), color));
                continue;
            }
            match characters.next() {
                Some(ESCAPE) => spans.push(Span::styled(ESCAPE.to_string(), color)),
                Some('0') => color = self.color,
                Some(code) => {
                    color = *self
                        .colors
                        .get(&code)
                        .ok_or_else(|| format!("uses ^{code}, which has no color (write ^^ for a plain ^)"))?;
                }
                None => return Err("has a line ending in a lone ^".into()),
            }
        }
        Ok(Line::from(spans))
    }

    pub fn size(&self) -> (u16, u16) {
        self.clips
            .values()
            .flat_map(|animation| &animation.frames)
            .map(|frame| self.paint(frame))
            .fold((0, 0), |(width, height), text| {
                (width.max(text.width() as u16), height.max(text.height() as u16))
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
    fn frame_at_follows_the_sequence_and_wraps() {
        let animation = Animation {
            frame_ms: 100,
            frames: vec!["a".into(), "b".into()],
            sequence: vec![0, 0, 1],
        };
        let at = |millis| animation.frame_at(Duration::from_millis(millis));
        assert_eq!([at(0), at(150), at(250), at(300)], ["a", "a", "b", "a"]);
    }

    #[test]
    fn escapes_switch_colors_until_reset_or_line_end() {
        let species = Species::parse(SAMPLE).unwrap();
        let painted: Vec<_> = species
            .paint("a^1b^0c^1d\ne^^")
            .iter()
            .flat_map(|line| line.spans.clone())
            .map(|span| (span.content.into_owned(), span.style.fg.unwrap()))
            .collect();
        let body = Color::Rgb(1, 1, 1);
        let red = Color::Rgb(2, 2, 2);
        let expected = [("a", body), ("b", red), ("c", body), ("d", red), ("e", body), ("^", body)];
        assert_eq!(painted, expected.map(|(text, color)| (text.to_string(), color)));
    }

    #[test]
    fn unknown_and_dangling_escapes_are_rejected() {
        for art in ["^2", "^x", "a^"] {
            let source = SAMPLE.replace("a^1b", art);
            assert!(Species::parse(&source).is_err(), "{art} was accepted");
        }
    }

    const SAMPLE: &str = r##"
species = "Sample"
color = "#010101"

[colors]
1 = "#020202"

[clips.idle]
frame_ms = 100
frames = ["a^1b"]
"##;
}
