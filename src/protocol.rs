//! Native-mode MIDI event decoding and LED message encoding.

use crate::control::{Button, Strip};

/// Something the user did on the device.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// Fader position, 0.0 (bottom) to 1.0 (top).
    Fader { strip: Strip, value: f32 },
    /// Knob position, 0.0 (fully left) to 1.0 (fully right).
    Knob { strip: Strip, value: f32 },
    /// A button was pressed or released.
    Button { button: Button, pressed: bool },
    /// A message not recognised as a native-mode control event.
    Unmapped(Vec<u8>),
}

// Native mode uses CC messages on channel 16.
const STATUS: u8 = 0xBF;
const FADER: u8 = 0x00;
const KNOB: u8 = 0x10;

/// Decodes one MIDI message from a device in native mode.
pub fn decode(msg: &[u8]) -> Event {
    decode_known(msg).unwrap_or_else(|| Event::Unmapped(msg.to_vec()))
}

fn decode_known(msg: &[u8]) -> Option<Event> {
    let &[STATUS, cc, value] = msg else {
        return None;
    };
    let strip = |base: u8| cc.checked_sub(base).and_then(Strip::new);
    let position = f32::from(value) / 127.0;
    if let Some(strip) = strip(FADER) {
        return Some(Event::Fader {
            strip,
            value: position,
        });
    }
    if let Some(strip) = strip(KNOB) {
        return Some(Event::Knob {
            strip,
            value: position,
        });
    }
    let button = Button::all().find(|b| cc_of(*b) == cc)?;
    Some(Event::Button {
        button,
        pressed: value > 0,
    })
}

/// Encodes an LED message. Buttons without LEDs ignore it.
pub fn led_message(button: Button, on: bool) -> [u8; 3] {
    [STATUS, cc_of(button), if on { 0x7F } else { 0x00 }]
}

const fn cc_of(button: Button) -> u8 {
    match button {
        Button::Solo(s) => 0x20 + s.index() as u8,
        Button::Mute(s) => 0x30 + s.index() as u8,
        Button::Rec(s) => 0x40 + s.index() as u8,
        Button::Play => 0x29,
        Button::Stop => 0x2A,
        Button::Rewind => 0x2B,
        Button::FastForward => 0x2C,
        Button::Record => 0x2D,
        Button::Cycle => 0x2E,
        Button::TrackPrev => 0x3A,
        Button::TrackNext => 0x3B,
        Button::MarkerSet => 0x3C,
        Button::MarkerPrev => 0x3D,
        Button::MarkerNext => 0x3E,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strip(i: u8) -> Strip {
        Strip::new(i).unwrap()
    }

    // Byte sequences captured from a nanoKONTROL2 in native mode.
    #[test]
    fn decodes_captured_messages() {
        assert_eq!(
            decode(&[0xBF, 0x04, 0x7F]),
            Event::Fader {
                strip: strip(4),
                value: 1.0
            }
        );
        assert_eq!(
            decode(&[0xBF, 0x11, 0x00]),
            Event::Knob {
                strip: strip(1),
                value: 0.0
            }
        );
        assert_eq!(
            decode(&[0xBF, 0x20, 0x7F]),
            Event::Button {
                button: Button::Solo(strip(0)),
                pressed: true
            }
        );
        assert_eq!(
            decode(&[0xBF, 0x41, 0x00]),
            Event::Button {
                button: Button::Rec(strip(1)),
                pressed: false
            }
        );
    }

    #[test]
    fn rejects_other_channels_and_unknown_ccs() {
        assert!(matches!(decode(&[0xB0, 0x00, 0x40]), Event::Unmapped(_)));
        assert!(matches!(decode(&[0xBF, 0x50, 0x40]), Event::Unmapped(_)));
    }

    #[test]
    fn every_button_round_trips() {
        for b in Button::all() {
            let [_, cc, _] = led_message(b, true);
            assert_eq!(
                decode(&[0xBF, cc, 0x7F]),
                Event::Button {
                    button: b,
                    pressed: true
                }
            );
        }
    }

    #[test]
    fn led_messages() {
        assert_eq!(led_message(Button::Play, true), [0xBF, 0x29, 0x7F]);
        assert_eq!(
            led_message(Button::Mute(strip(7)), false),
            [0xBF, 0x37, 0x00]
        );
    }
}
