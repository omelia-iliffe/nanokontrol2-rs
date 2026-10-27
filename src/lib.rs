//! Korg nanoKONTROL2 driver: typed events, LED control, and a mirror of the control state.
//!
//! [`NanoKontrol2::connect`] puts the device in Korg's native mode, in which every control
//! sends a fixed message and every LED follows the computer. This works regardless of the
//! current mode or settings and leaves the stored settings unchanged. Dropping the connection
//! returns the device to its normal mode, as does unplugging it.
//!
//! [`decode`] and [`led_message`] are independent of any MIDI backend, with the [`sysex`]
//! module handling native mode. The default `midir` feature adds device discovery and
//! connections through [`NanoKontrol2`].
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

#![forbid(unsafe_code)]
// The crate docs link to the midir-only device API.
#![cfg_attr(not(feature = "midir"), allow(rustdoc::broken_intra_doc_links))]

mod control;
#[cfg(feature = "midir")]
mod device;
mod protocol;
mod state;
pub mod sysex;

pub use control::{Button, Strip};
#[cfg(feature = "midir")]
pub use device::{Error, ExitHandle, NanoKontrol2};
#[cfg(feature = "midir")]
pub use midir;
pub use protocol::{Event, decode, led_message};
pub use state::State;

#[cfg(all(doctest, feature = "midir"))]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;
