//! Connecting to the device through `midir`.

use std::fmt;
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use midir::{
    Ignore, MidiInput, MidiInputConnection, MidiInputPort, MidiOutput, MidiOutputConnection,
    MidiOutputPort,
};

use crate::control::Button;
use crate::protocol::{self, Event};
use crate::state::State;
use crate::sysex::{self, Reply};

const CLIENT_NAME: &str = "nanokontrol2-rs";
const SYSEX_TIMEOUT: Duration = Duration::from_secs(2);

/// Errors from [`NanoKontrol2`].
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// No MIDI port whose name contains `name`.
    PortNotFound {
        /// Whether the missing port is the input rather than the output.
        input: bool,
        name: String,
    },
    Init(midir::InitError),
    Connect(String),
    Send(midir::SendError),
    /// The device did not answer a SysEx request.
    Timeout,
    /// The device answered a SysEx request with an error or something unexpected.
    Rejected(Reply),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::PortNotFound { input, name } => {
                let dir = if *input { "input" } else { "output" };
                write!(f, "no MIDI {dir} port matching {name:?}")
            }
            Error::Init(e) => write!(f, "MIDI init failed: {e}"),
            Error::Connect(e) => write!(f, "MIDI connect failed: {e}"),
            Error::Send(e) => write!(f, "MIDI send failed: {e}"),
            Error::Timeout => write!(f, "device did not answer the SysEx request"),
            Error::Rejected(reply) => write!(f, "device rejected the SysEx request: {reply:?}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Init(e) => Some(e),
            Error::Send(e) => Some(e),
            _ => None,
        }
    }
}

impl From<midir::InitError> for Error {
    fn from(e: midir::InitError) -> Self {
        Error::Init(e)
    }
}

impl From<midir::SendError> for Error {
    fn from(e: midir::SendError) -> Self {
        Error::Send(e)
    }
}

/// A connected nanoKONTROL2, held in native mode.
///
/// Events arrive on the [`Receiver`] returned by [`NanoKontrol2::connect`]. [`state`](Self::state)
/// is updated before each event is sent, so it always includes every event already received.
///
/// Dropping this leaves native mode and closes the connection. Terminating the process without
/// running `Drop`, as the default Ctrl-C action does, leaves native mode active; see
/// [`exit_handle`](Self::exit_handle).
pub struct NanoKontrol2 {
    _input: MidiInputConnection<()>,
    output: Arc<Mutex<MidiOutputConnection>>,
    state: Arc<Mutex<State>>,
    replies: Receiver<Reply>,
    global_channel: u8,
}

impl Drop for NanoKontrol2 {
    fn drop(&mut self) {
        self.exit_handle().leave_native();
    }
}

impl fmt::Debug for NanoKontrol2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NanoKontrol2").finish_non_exhaustive()
    }
}

impl NanoKontrol2 {
    /// The port name [`connect`](Self::connect) looks for, matched case-insensitively.
    pub const PORT_NAME: &str = "nanoKONTROL2";

    /// Connects to the first input and output ports whose names contain
    /// [`PORT_NAME`](Self::PORT_NAME) and puts the device in native mode.
    pub fn connect() -> Result<(Self, Receiver<Event>), Error> {
        Self::connect_matching(Self::PORT_NAME)
    }

    /// Like [`connect`](Self::connect), for the first ports whose names contain `name`,
    /// case-insensitively. Useful with more than one device attached.
    pub fn connect_matching(name: &str) -> Result<(Self, Receiver<Event>), Error> {
        let midi_in = MidiInput::new(CLIENT_NAME)?;
        let midi_out = MidiOutput::new(CLIENT_NAME)?;
        let in_port =
            find_port(&midi_in, &midi_in.ports(), name).ok_or_else(|| Error::PortNotFound {
                input: true,
                name: name.into(),
            })?;
        let out_port =
            find_port(&midi_out, &midi_out.ports(), name).ok_or_else(|| Error::PortNotFound {
                input: false,
                name: name.into(),
            })?;
        Self::connect_ports(midi_in, &in_port, midi_out, &out_port)
    }

    /// Connects to the given ports and puts the device in native mode.
    pub fn connect_ports(
        mut midi_in: MidiInput,
        in_port: &MidiInputPort,
        midi_out: MidiOutput,
        out_port: &MidiOutputPort,
    ) -> Result<(Self, Receiver<Event>), Error> {
        let state = Arc::new(Mutex::new(State::default()));
        let (tx, rx) = mpsc::channel();
        let (reply_tx, replies) = mpsc::channel();
        midi_in.ignore(Ignore::None);

        let input = {
            let state = Arc::clone(&state);
            midi_in
                .connect(
                    in_port,
                    CLIENT_NAME,
                    move |_timestamp, msg, _| {
                        if let Some(reply) = sysex::parse_reply(msg) {
                            let _ = reply_tx.send(reply);
                            return;
                        }
                        let event = protocol::decode(msg);
                        lock(&state).apply(&event);
                        // A dropped receiver is fine: the state mirror still updates.
                        let _ = tx.send(event);
                    },
                    (),
                )
                .map_err(|e| Error::Connect(e.to_string()))?
        };
        let output = midi_out
            .connect(out_port, CLIENT_NAME)
            .map_err(|e| Error::Connect(e.to_string()))?;

        let mut nk = NanoKontrol2 {
            _input: input,
            output: Arc::new(Mutex::new(output)),
            state,
            replies,
            global_channel: 0,
        };
        nk.enter_native()?;
        Ok((nk, rx))
    }

    /// A snapshot of the controls.
    pub fn state(&self) -> State {
        lock(&self.state).clone()
    }

    /// Switches `button`'s LED.
    pub fn set_led(&mut self, button: Button, on: bool) -> Result<(), Error> {
        self.send_raw(&protocol::led_message(button, on))?;
        lock(&self.state).leds[button.index()] = on;
        Ok(())
    }

    /// Switches every LED.
    pub fn set_all_leds(&mut self, on: bool) -> Result<(), Error> {
        Button::all()
            .filter(|b| b.has_led())
            .try_for_each(|b| self.set_led(b, on))
    }

    /// Sends a raw MIDI message, for anything the typed API does not cover.
    pub fn send_raw(&mut self, msg: &[u8]) -> Result<(), Error> {
        Ok(lock(&self.output).send(msg)?)
    }

    /// Returns a handle for leaving native mode from a shutdown handler.
    pub fn exit_handle(&self) -> ExitHandle {
        ExitHandle {
            output: Arc::clone(&self.output),
            global_channel: self.global_channel,
        }
    }

    fn enter_native(&mut self) -> Result<(), Error> {
        self.global_channel = match self.request(&sysex::search_device_request(0))? {
            Reply::SearchDevice { global_channel } => global_channel,
            other => return Err(Error::Rejected(other)),
        };
        // The device refuses to enter native mode while already in it.
        match self.request(&sysex::mode_request(self.global_channel))? {
            Reply::NativeMode(true) => return Ok(()),
            Reply::NativeMode(false) => {}
            other => return Err(Error::Rejected(other)),
        }
        match self.request(&sysex::native_mode_request(self.global_channel, true))? {
            Reply::NativeMode(true) => Ok(()),
            other => Err(Error::Rejected(other)),
        }
    }

    /// Sends a SysEx request and returns the device's first reply.
    fn request(&mut self, msg: &[u8]) -> Result<Reply, Error> {
        // Discard replies to earlier requests that timed out.
        while self.replies.try_recv().is_ok() {}
        self.send_raw(msg)?;
        self.replies
            .recv_timeout(SYSEX_TIMEOUT)
            .map_err(|_| Error::Timeout)
    }
}

/// A handle for leaving native mode; see [`NanoKontrol2::exit_handle`].
#[derive(Clone)]
pub struct ExitHandle {
    output: Arc<Mutex<MidiOutputConnection>>,
    global_channel: u8,
}

impl fmt::Debug for ExitHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExitHandle").finish_non_exhaustive()
    }
}

impl ExitHandle {
    /// Attempts to leave native mode without waiting for a reply.
    pub fn leave_native(&self) {
        let msg = sysex::native_mode_request(self.global_channel, false);
        let _ = lock(&self.output).send(&msg);
    }
}

fn find_port<T: midir::MidiIO>(io: &T, ports: &[T::Port], name: &str) -> Option<T::Port> {
    let name = name.to_lowercase();
    ports
        .iter()
        .find(|p| {
            io.port_name(p)
                .is_ok_and(|n| n.to_lowercase().contains(&name))
        })
        .cloned()
}

/// Ignores poisoning: the guarded values are plain data a panic cannot leave inconsistent.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
