//! The physical controls on the nanoKONTROL2.

use std::fmt;

/// One of the eight channel strips (knob, fader, and S/M/R buttons), indexed 0 to 7 from the left.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Strip(u8);

impl Strip {
    /// Number of strips.
    pub const COUNT: usize = 8;

    /// Returns `None` if `index` is 8 or more.
    pub const fn new(index: u8) -> Option<Self> {
        if (index as usize) < Self::COUNT {
            Some(Self(index))
        } else {
            None
        }
    }

    /// 0 to 7, from the left.
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    /// Every strip, from the left.
    pub fn all() -> impl Iterator<Item = Strip> {
        (0..Self::COUNT as u8).map(Strip)
    }
}

impl fmt::Display for Strip {
    /// Formats as the 1-based number printed on the device.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0 + 1)
    }
}

/// Every button on the device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Button {
    Solo(Strip),
    Mute(Strip),
    Rec(Strip),
    TrackPrev,
    TrackNext,
    Cycle,
    MarkerSet,
    MarkerPrev,
    MarkerNext,
    Rewind,
    FastForward,
    Stop,
    Play,
    Record,
}

const TRANSPORT: [Button; 11] = [
    Button::TrackPrev,
    Button::TrackNext,
    Button::Cycle,
    Button::MarkerSet,
    Button::MarkerPrev,
    Button::MarkerNext,
    Button::Rewind,
    Button::FastForward,
    Button::Stop,
    Button::Play,
    Button::Record,
];

impl Button {
    /// Number of buttons.
    pub const COUNT: usize = 3 * Strip::COUNT + TRANSPORT.len();

    /// A dense index in `0..Button::COUNT`, for per-button tables.
    pub const fn index(self) -> usize {
        match self {
            Button::Solo(s) => s.index(),
            Button::Mute(s) => Strip::COUNT + s.index(),
            Button::Rec(s) => 2 * Strip::COUNT + s.index(),
            Button::TrackPrev => 24,
            Button::TrackNext => 25,
            Button::Cycle => 26,
            Button::MarkerSet => 27,
            Button::MarkerPrev => 28,
            Button::MarkerNext => 29,
            Button::Rewind => 30,
            Button::FastForward => 31,
            Button::Stop => 32,
            Button::Play => 33,
            Button::Record => 34,
        }
    }

    /// Inverse of [`Button::index`].
    pub const fn from_index(index: usize) -> Option<Self> {
        const S: usize = Strip::COUNT;
        match index {
            i if i < S => Some(Button::Solo(Strip(i as u8))),
            i if i < 2 * S => Some(Button::Mute(Strip((i - S) as u8))),
            i if i < 3 * S => Some(Button::Rec(Strip((i - 2 * S) as u8))),
            i if i < Self::COUNT => Some(TRANSPORT[i - 3 * S]),
            _ => None,
        }
    }

    /// Every button, in [`Button::index`] order.
    pub fn all() -> impl Iterator<Item = Button> {
        (0..Self::COUNT).filter_map(Button::from_index)
    }

    /// The TRACK and MARKER buttons have no LED.
    pub const fn has_led(self) -> bool {
        !matches!(
            self,
            Button::TrackPrev
                | Button::TrackNext
                | Button::MarkerSet
                | Button::MarkerPrev
                | Button::MarkerNext
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_round_trips() {
        for i in 0..Button::COUNT {
            assert_eq!(Button::from_index(i).unwrap().index(), i);
        }
        assert_eq!(Button::from_index(Button::COUNT), None);
    }
}
