//! Decoding the device's CC-mode MIDI into [`Event`]s and encoding LED messages.

use crate::control::{Button, Strip};
use crate::sysex::Scene;

/// Something the user did on the device.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// Fader position, 0.0 (bottom) to 1.0 (top).
    Fader { strip: Strip, value: f32 },
    /// Knob position, 0.0 (fully left) to 1.0 (fully right).
    Knob { strip: Strip, value: f32 },
    /// With a button set to Toggle behaviour, `pressed` is the toggle state.
    Button { button: Button, pressed: bool },
    /// A message with no assignment in the [`Map`].
    Unmapped(Vec<u8>),
}

const NOTE_OFF: u8 = 0x80;
const NOTE_ON: u8 = 0x90;
const CONTROL_CHANGE: u8 = 0xB0;

/// A fader or knob's assignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ControlAssign {
    /// MIDI channel, 0 to 15 (the device's "1 to 16").
    pub channel: u8,
    pub cc: u8,
    /// The values sent at the bottom (or fully left) and top (or fully right). `min` may be
    /// above `max`, which reverses the control.
    pub min: u8,
    pub max: u8,
}

impl ControlAssign {
    fn factory(cc: u8) -> Self {
        ControlAssign {
            channel: 0,
            cc,
            min: 0,
            max: 127,
        }
    }

    fn normalise(&self, value: u8) -> f32 {
        if self.min == self.max {
            return 0.0;
        }
        let (v, lo, hi) = (f32::from(value), f32::from(self.min), f32::from(self.max));
        ((v - lo) / (hi - lo)).clamp(0.0, 1.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignType {
    ControlChange,
    Note,
}

/// A button's assignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ButtonAssign {
    /// MIDI channel, 0 to 15 (the device's "1 to 16").
    pub channel: u8,
    pub kind: AssignType,
    /// CC or note number.
    pub number: u8,
    /// The CC values for off and on, or for a note the on velocity (`off` is unused).
    pub off: u8,
    pub on: u8,
}

impl ButtonAssign {
    fn factory(cc: u8) -> Self {
        ButtonAssign {
            channel: 0,
            kind: AssignType::ControlChange,
            number: cc,
            off: 0,
            on: 127,
        }
    }
}

/// Every control's assignment; `None` is a control set to Disable or No Assign. [`Default`] is the
/// factory scene. [`Map::from_scene`] reads a scene edited in Korg Kontrol Editor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Map {
    pub fader: [Option<ControlAssign>; Strip::COUNT],
    pub knob: [Option<ControlAssign>; Strip::COUNT],
    /// Indexed by [`Button::index`].
    pub button: [Option<ButtonAssign>; Button::COUNT],
}

impl Default for Map {
    fn default() -> Self {
        let mut button = [None; Button::COUNT];
        for s in Strip::all() {
            let i = s.index() as u8;
            button[Button::Solo(s).index()] = Some(ButtonAssign::factory(32 + i));
            button[Button::Mute(s).index()] = Some(ButtonAssign::factory(48 + i));
            button[Button::Rec(s).index()] = Some(ButtonAssign::factory(64 + i));
        }
        for (b, cc) in [
            (Button::TrackPrev, 58),
            (Button::TrackNext, 59),
            (Button::Cycle, 46),
            (Button::MarkerSet, 60),
            (Button::MarkerPrev, 61),
            (Button::MarkerNext, 62),
            (Button::Rewind, 43),
            (Button::FastForward, 44),
            (Button::Stop, 42),
            (Button::Play, 41),
            (Button::Record, 45),
        ] {
            button[b.index()] = Some(ButtonAssign::factory(cc));
        }
        Map {
            fader: std::array::from_fn(|i| Some(ControlAssign::factory(i as u8))),
            knob: std::array::from_fn(|i| Some(ControlAssign::factory(16 + i as u8))),
            button,
        }
    }
}

// Offsets into a scene, from TABLE 1 of Korg's MIDI Implementation.
const GROUP_START: usize = 3;
const GROUP_LEN: usize = 31;
const GROUP_SLIDER: usize = 1;
const GROUP_KNOB: usize = 7;
const GROUP_SOLO: usize = 13;
const GROUP_MUTE: usize = 19;
const GROUP_REC: usize = 25;
const TRANSPORT_CHANNEL: usize = 251;
const TRANSPORT_START: usize = 252;
const TRANSPORT_LEN: usize = 6;
/// A group or transport channel of 16 means "use the global channel".
const USE_GLOBAL_CHANNEL: u8 = 16;

impl Map {
    /// The assignments in `scene`. Toggle behaviour is not recorded here: a toggled button
    /// reports its on and off values as presses and releases.
    pub fn from_scene(scene: &Scene) -> Self {
        let d = &scene.0;
        let global = scene.global_channel();
        let channel = |c: u8| if c >= USE_GLOBAL_CHANNEL { global } else { c };
        // Faders and knobs: enable, reserved, CC, min, max.
        let control = |at: usize, ch: u8| {
            (d[at] != 0).then(|| ControlAssign {
                channel: ch,
                cc: d[at + 2],
                min: d[at + 3],
                max: d[at + 4],
            })
        };
        // Buttons: assign type, behaviour, number, off, on.
        let button = |at: usize, ch: u8| {
            let kind = match d[at] {
                1 => AssignType::ControlChange,
                2 => AssignType::Note,
                _ => return None,
            };
            Some(ButtonAssign {
                channel: ch,
                kind,
                number: d[at + 2],
                off: d[at + 3],
                on: d[at + 4],
            })
        };

        let mut map = Map {
            fader: [None; Strip::COUNT],
            knob: [None; Strip::COUNT],
            button: [None; Button::COUNT],
        };
        for s in Strip::all() {
            let g = GROUP_START + GROUP_LEN * s.index();
            let ch = channel(d[g]);
            map.fader[s.index()] = control(g + GROUP_SLIDER, ch);
            map.knob[s.index()] = control(g + GROUP_KNOB, ch);
            map.button[Button::Solo(s).index()] = button(g + GROUP_SOLO, ch);
            map.button[Button::Mute(s).index()] = button(g + GROUP_MUTE, ch);
            map.button[Button::Rec(s).index()] = button(g + GROUP_REC, ch);
        }
        // The scene lists the transport buttons in the same order as Button::index.
        let ch = channel(d[TRANSPORT_CHANNEL]);
        for t in 0..Button::COUNT - 3 * Strip::COUNT {
            let b = Button::from_index(3 * Strip::COUNT + t).expect("transport index in range");
            map.button[b.index()] = button(TRANSPORT_START + TRANSPORT_LEN * t, ch);
        }
        map
    }

    pub fn decode(&self, msg: &[u8]) -> Event {
        self.decode_mapped(msg)
            .unwrap_or_else(|| Event::Unmapped(msg.to_vec()))
    }

    fn decode_mapped(&self, msg: &[u8]) -> Option<Event> {
        let &[status, number, value] = msg else {
            return None;
        };
        let channel = status & 0x0F;
        let (kind, pressed) = match status & 0xF0 {
            CONTROL_CHANGE => {
                let at = |c: &ControlAssign| c.channel == channel && c.cc == number;
                if let Some(i) = self.fader.iter().position(|c| c.as_ref().is_some_and(at)) {
                    let c = self.fader[i]?;
                    let strip = Strip::new(i as u8)?;
                    return Some(Event::Fader {
                        strip,
                        value: c.normalise(value),
                    });
                }
                if let Some(i) = self.knob.iter().position(|c| c.as_ref().is_some_and(at)) {
                    let c = self.knob[i]?;
                    let strip = Strip::new(i as u8)?;
                    return Some(Event::Knob {
                        strip,
                        value: c.normalise(value),
                    });
                }
                (AssignType::ControlChange, None)
            }
            NOTE_ON => (AssignType::Note, Some(value > 0)),
            NOTE_OFF => (AssignType::Note, Some(false)),
            _ => return None,
        };
        let i = self.button.iter().position(|b| {
            b.is_some_and(|b| b.channel == channel && b.kind == kind && b.number == number)
        })?;
        let b = self.button[i]?;
        let pressed = pressed.unwrap_or(value != b.off);
        Some(Event::Button {
            button: Button::from_index(i)?,
            pressed,
        })
    }

    /// The message that switches `button`'s LED on or off, or `None` if the button is
    /// unassigned. The device only responds with its LED Mode set to External.
    pub fn led_message(&self, button: Button, on: bool) -> Option<[u8; 3]> {
        let b = self.button[button.index()]?;
        Some(match b.kind {
            AssignType::ControlChange => [
                CONTROL_CHANGE | b.channel,
                b.number,
                if on { b.on } else { b.off },
            ],
            AssignType::Note if on => [NOTE_ON | b.channel, b.number, b.on.max(1)],
            AssignType::Note => [NOTE_OFF | b.channel, b.number, 0],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strip(i: u8) -> Strip {
        Strip::new(i).unwrap()
    }

    #[test]
    fn cc_factory_scene() {
        let p = Map::default();
        assert_eq!(
            p.decode(&[0xB0, 2, 127]),
            Event::Fader {
                strip: strip(2),
                value: 1.0
            }
        );
        assert_eq!(
            p.decode(&[0xB0, 23, 0]),
            Event::Knob {
                strip: strip(7),
                value: 0.0
            }
        );
        assert_eq!(
            p.decode(&[0xB0, 49, 127]),
            Event::Button {
                button: Button::Mute(strip(1)),
                pressed: true
            }
        );
        assert_eq!(
            p.decode(&[0xB0, 41, 0]),
            Event::Button {
                button: Button::Play,
                pressed: false
            }
        );
    }

    #[test]
    fn cc_ignores_other_channels() {
        assert_eq!(
            Map::default().decode(&[0xB1, 0, 64]),
            Event::Unmapped(vec![0xB1, 0, 64])
        );
    }

    #[test]
    fn cc_led() {
        let p = Map::default();
        assert_eq!(
            p.led_message(Button::Rec(strip(3)), true),
            Some([0xB0, 67, 127])
        );
        assert_eq!(p.led_message(Button::Cycle, false), Some([0xB0, 46, 0]));
    }

    #[test]
    fn cc_map_from_factory_scene_matches_default() {
        // The factory scene, laid out per TABLE 1.
        let mut d = [0; crate::sysex::SCENE_LEN];
        let mut put = |at: usize, vals: &[u8]| d[at..at + vals.len()].copy_from_slice(vals);
        for g in 0..8u8 {
            let at = 3 + 31 * usize::from(g);
            put(at, &[16, 1, 0, g, 0, 127, 0, 1, 0, 16 + g, 0, 127, 0]);
            put(at + 13, &[1, 0, 32 + g, 0, 127, 0]);
            put(at + 19, &[1, 0, 48 + g, 0, 127, 0]);
            put(at + 25, &[1, 0, 64 + g, 0, 127, 0]);
        }
        put(251, &[16]);
        for (t, cc) in [58u8, 59, 46, 60, 61, 62, 43, 44, 42, 41, 45]
            .into_iter()
            .enumerate()
        {
            put(252 + 6 * t, &[1, 0, cc, 0, 127, 0]);
        }
        let scene = Scene(d);
        assert_eq!(Map::from_scene(&scene), Map::default());
    }

    #[test]
    fn cc_custom_assignments() {
        let mut map = Map::default();
        map.fader[0] = Some(ControlAssign {
            channel: 2,
            cc: 7,
            min: 127,
            max: 0,
        });
        map.knob[1] = None;
        map.button[Button::Play.index()] = Some(ButtonAssign {
            channel: 9,
            kind: AssignType::Note,
            number: 60,
            off: 0,
            on: 100,
        });
        let p = map;
        assert_eq!(
            p.decode(&[0xB2, 7, 127]),
            Event::Fader {
                strip: strip(0),
                value: 0.0
            }
        );
        assert_eq!(
            p.decode(&[0xB0, 17, 64]),
            Event::Unmapped(vec![0xB0, 17, 64])
        );
        assert_eq!(
            p.decode(&[0x99, 60, 100]),
            Event::Button {
                button: Button::Play,
                pressed: true
            }
        );
        assert_eq!(
            p.decode(&[0x89, 60, 64]),
            Event::Button {
                button: Button::Play,
                pressed: false
            }
        );
        assert_eq!(p.led_message(Button::Play, true), Some([0x99, 60, 100]));
    }
}
