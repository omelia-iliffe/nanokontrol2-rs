use crate::control::{Button, Strip};
use crate::map::Event;

/// A mirror of the device's controls, built from the events seen so far.
///
/// The device does not report its controls' positions on connect, so a fader or knob is
/// `None` until it is first moved.
#[derive(Debug, Clone, PartialEq)]
pub struct State {
    /// Indexed by [`Strip::index`], 0.0 to 1.0.
    pub faders: [Option<f32>; Strip::COUNT],
    /// Indexed by [`Strip::index`], 0.0 to 1.0.
    pub knobs: [Option<f32>; Strip::COUNT],
    /// Indexed by [`Button::index`].
    pub pressed: [bool; Button::COUNT],
    /// The LED states last sent to the device, indexed by [`Button::index`]. With LED Mode set
    /// to Internal, the device drives its own LEDs and this does not reflect them.
    pub leds: [bool; Button::COUNT],
}

impl Default for State {
    fn default() -> Self {
        State {
            faders: [None; Strip::COUNT],
            knobs: [None; Strip::COUNT],
            pressed: [false; Button::COUNT],
            leds: [false; Button::COUNT],
        }
    }
}

impl State {
    /// Updates the mirror with `event`.
    pub fn apply(&mut self, event: &Event) {
        match *event {
            Event::Fader { strip, value } => self.faders[strip.index()] = Some(value),
            Event::Knob { strip, value } => self.knobs[strip.index()] = Some(value),
            Event::Button { button, pressed } => self.pressed[button.index()] = pressed,
            Event::Unmapped(_) => {}
        }
    }

    /// `None` until the fader is first moved.
    pub fn fader(&self, strip: Strip) -> Option<f32> {
        self.faders[strip.index()]
    }

    /// `None` until the knob is first moved.
    pub fn knob(&self, strip: Strip) -> Option<f32> {
        self.knobs[strip.index()]
    }

    /// Whether `button` is held, or on if it is set to Toggle.
    pub fn is_pressed(&self, button: Button) -> bool {
        self.pressed[button.index()]
    }

    /// The LED state last sent.
    pub fn led(&self, button: Button) -> bool {
        self.leds[button.index()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applies_events() {
        let s2 = Strip::new(2).unwrap();
        let mut state = State::default();
        state.apply(&Event::Fader {
            strip: s2,
            value: 0.5,
        });
        state.apply(&Event::Knob {
            strip: s2,
            value: 0.25,
        });
        state.apply(&Event::Button {
            button: Button::Play,
            pressed: true,
        });
        assert_eq!(state.fader(s2), Some(0.5));
        assert_eq!(state.fader(Strip::new(0).unwrap()), None);
        assert_eq!(state.knob(s2), Some(0.25));
        assert!(state.is_pressed(Button::Play));
        assert!(!state.is_pressed(Button::Stop));
    }
}
