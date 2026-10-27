//! Korg nanoKONTROL2 driver for CC mode: typed events, LED control, and a mirror of the
//! control state.
//!
//! [`NanoKontrol2::connect`] reads the device's scene over SysEx and decodes with its CC
//! assignments, so scenes edited in Korg Kontrol Editor work unchanged. The device must be in
//! CC mode (plug it in while holding SET MARKER and CYCLE, or call
//! [`NanoKontrol2::set_control_mode`]); its DAW modes are not decoded.
//!
//! LEDs only follow the computer when the scene's LED Mode is External, which
//! [`NanoKontrol2::set_led_mode`] can switch. At the factory setting, Internal, the device
//! lights them itself and ignores [`NanoKontrol2::set_led`].
//!
//! [`Map`] is independent of any MIDI backend: feed raw messages to [`Map::decode`] and
//! send the bytes from [`Map::led_message`] yourself. The default `midir` feature adds
//! [`NanoKontrol2`], which finds the device and does that for you.
//!
//! ```no_run
//! # #[cfg(feature = "midir")] {
//! use nanokontrol2::{Event, NanoKontrol2};
//!
//! let (mut nk, events) = NanoKontrol2::connect()?;
//! for event in events {
//!     if let Event::Button { button, pressed: true } = event {
//!         let lit = nk.state().led(button);
//!         nk.set_led(button, !lit)?;
//!     }
//! }
//! # }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod control;
#[cfg(feature = "midir")]
mod device;
mod map;
mod state;
pub mod sysex;

pub use control::{Button, Strip};
#[cfg(feature = "midir")]
pub use device::{Error, NanoKontrol2};
pub use map::{AssignType, ButtonAssign, ControlAssign, Event, Map};
#[cfg(feature = "midir")]
pub use midir;
pub use state::State;
