//! Connecting to the device through `midir`.

use std::fmt;
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use midir::{
    Ignore, MidiInput, MidiInputConnection, MidiInputPort, MidiOutput, MidiOutputConnection,
    MidiOutputPort,
};

use crate::Event;
use crate::control::Button;
use crate::map::Map;
use crate::state::State;
use crate::sysex::{self, ControlMode, LedMode, Reply, Scene};

const CLIENT_NAME: &str = "nanokontrol2-rs";
const PORT_MATCH: &str = "nanokontrol2";
const SYSEX_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug)]
pub enum Error {
    /// No MIDI port whose name contains the search string.
    PortNotFound {
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
    /// The device is in one of its DAW modes, which this crate does not decode. `None` is a
    /// mode value outside Korg's documented range.
    NotCcMode(Option<ControlMode>),
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
            Error::NotCcMode(mode) => write!(
                f,
                "device is in {mode:?} mode, not CC mode; plug it in while holding SET MARKER \
                 and CYCLE, or call set_control_mode"
            ),
        }
    }
}

impl std::error::Error for Error {}

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

/// A connected nanoKONTROL2.
///
/// Events arrive on the [`Receiver`] returned by [`NanoKontrol2::connect`]. [`state`](Self::state)
/// is updated before each event is sent, so it always includes every event already received.
/// Dropping this closes the connection and ends the event stream.
pub struct NanoKontrol2 {
    _input: MidiInputConnection<()>,
    output: MidiOutputConnection,
    map: Arc<Mutex<Map>>,
    state: Arc<Mutex<State>>,
    replies: Receiver<Reply>,
}

impl NanoKontrol2 {
    /// Connects to the first input and output ports whose names contain "nanoKONTROL2", then
    /// [`load_scene`](Self::load_scene).
    pub fn connect() -> Result<(Self, Receiver<Event>), Error> {
        Self::connect_matching(PORT_MATCH)
    }

    /// Like [`connect`](Self::connect), for the first ports whose names contain `name`,
    /// case-insensitively. Useful with more than one device attached.
    pub fn connect_matching(name: &str) -> Result<(Self, Receiver<Event>), Error> {
        let (mut nk, events) = Self::connect_unchecked(name)?;
        nk.load_scene()?;
        Ok((nk, events))
    }

    /// Connects without reading the device's scene, decoding with the factory [`Map`]. Use
    /// this to reach a device that is not in CC mode, to [`set_control_mode`](Self::set_control_mode).
    pub fn connect_unchecked(name: &str) -> Result<(Self, Receiver<Event>), Error> {
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

    /// Connects to the given ports, decoding with the factory [`Map`] until
    /// [`load_scene`](Self::load_scene) or [`set_map`](Self::set_map).
    pub fn connect_ports(
        mut midi_in: MidiInput,
        in_port: &MidiInputPort,
        midi_out: MidiOutput,
        out_port: &MidiOutputPort,
    ) -> Result<(Self, Receiver<Event>), Error> {
        let map = Arc::new(Mutex::new(Map::default()));
        let state = Arc::new(Mutex::new(State::default()));
        let (tx, rx) = mpsc::channel();
        let (reply_tx, replies) = mpsc::channel();
        midi_in.ignore(Ignore::None);

        let input = {
            let map = Arc::clone(&map);
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
                        let event = lock(&map).decode(msg);
                        lock(&state).apply(&event);
                        // The receiver being dropped only means nobody wants events; the
                        // state mirror is still kept current.
                        let _ = tx.send(event);
                    },
                    (),
                )
                .map_err(|e| Error::Connect(e.to_string()))?
        };
        let output = midi_out
            .connect(out_port, CLIENT_NAME)
            .map_err(|e| Error::Connect(e.to_string()))?;

        Ok((
            NanoKontrol2 {
                _input: input,
                output,
                map,
                state,
                replies,
            },
            rx,
        ))
    }

    pub fn map(&self) -> Map {
        lock(&self.map).clone()
    }

    /// Changes how incoming MIDI is decoded and LED messages are encoded.
    pub fn set_map(&mut self, map: Map) {
        *lock(&self.map) = map;
    }

    /// Reads the device's scene and decodes with its assignments from now on. Fails with
    /// [`Error::NotCcMode`] if the device is in a DAW mode.
    pub fn load_scene(&mut self) -> Result<(), Error> {
        let (_, scene) = self.read_scene()?;
        match scene.control_mode() {
            Some(ControlMode::Cc) => {
                self.set_map(Map::from_scene(&scene));
                Ok(())
            }
            mode => Err(Error::NotCcMode(mode)),
        }
    }

    /// A snapshot of the controls.
    pub fn state(&self) -> State {
        lock(&self.state).clone()
    }

    pub fn set_led(&mut self, button: Button, on: bool) -> Result<(), Error> {
        let msg = lock(&self.map).led_message(button, on);
        if let Some(msg) = msg {
            self.output.send(&msg)?;
        }
        lock(&self.state).leds[button.index()] = on;
        Ok(())
    }

    pub fn set_all_leds(&mut self, on: bool) -> Result<(), Error> {
        Button::all()
            .filter(|b| b.has_led())
            .try_for_each(|b| self.set_led(b, on))
    }

    /// Sends a raw MIDI message, for anything the typed API does not cover.
    pub fn send_raw(&mut self, msg: &[u8]) -> Result<(), Error> {
        Ok(self.output.send(msg)?)
    }

    /// Switches the LED mode; [`LedMode::External`] is needed for [`set_led`](Self::set_led)
    /// to work. With `save` false the change lasts until the device is unplugged; with `save`
    /// true the device's current scene is written to its memory, even if it was already in
    /// `mode`.
    pub fn set_led_mode(&mut self, mode: LedMode, save: bool) -> Result<(), Error> {
        self.edit_scene(save, |scene| scene.set_led_mode(mode))
    }

    /// Switches the device's operating mode, with `save` as for
    /// [`set_led_mode`](Self::set_led_mode). Only [`ControlMode::Cc`] is decoded by this crate.
    pub fn set_control_mode(&mut self, mode: ControlMode, save: bool) -> Result<(), Error> {
        self.edit_scene(save, |scene| scene.set_control_mode(mode))
    }

    /// Applies `edit` to the device's scene, writing it back only if it changed.
    fn edit_scene(&mut self, save: bool, edit: impl FnOnce(&mut Scene)) -> Result<(), Error> {
        let (channel, mut scene) = self.read_scene()?;
        let before = scene.clone();
        edit(&mut scene);
        if scene != before {
            self.write_scene(channel, &scene, false)?;
        }
        if save {
            self.save_scene(channel)?;
        }
        Ok(())
    }

    /// Reads the device's current scene, and the global MIDI channel it must be addressed with.
    pub fn read_scene(&mut self) -> Result<(u8, Scene), Error> {
        let channel = self.global_channel()?;
        match self.request(&sysex::scene_dump_request(channel))? {
            Reply::Scene(scene) => Ok((channel, *scene)),
            other => Err(Error::Rejected(other)),
        }
    }

    /// Replaces the device's current scene, saving it to the device's memory if `save` is true.
    pub fn write_scene(&mut self, channel: u8, scene: &Scene, save: bool) -> Result<(), Error> {
        expect(
            self.request(&sysex::scene_dump(channel, scene))?,
            Reply::LoadCompleted,
        )?;
        if save {
            self.save_scene(channel)?;
        }
        Ok(())
    }

    /// Writes the device's current scene to its memory.
    pub fn save_scene(&mut self, channel: u8) -> Result<(), Error> {
        expect(
            self.request(&sysex::scene_write_request(channel))?,
            Reply::WriteCompleted,
        )
    }

    fn global_channel(&mut self) -> Result<u8, Error> {
        match self.request(&sysex::search_device_request(0))? {
            Reply::SearchDevice { global_channel } => Ok(global_channel),
            other => Err(Error::Rejected(other)),
        }
    }

    /// Sends a SysEx request and returns the device's first reply.
    fn request(&mut self, msg: &[u8]) -> Result<Reply, Error> {
        // Discard replies to earlier requests that timed out.
        while self.replies.try_recv().is_ok() {}
        self.output.send(msg)?;
        self.replies
            .recv_timeout(SYSEX_TIMEOUT)
            .map_err(|_| Error::Timeout)
    }
}

fn expect(reply: Reply, wanted: Reply) -> Result<(), Error> {
    if reply == wanted {
        Ok(())
    } else {
        Err(Error::Rejected(reply))
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

/// The state is plain data with no invariants a panicking holder could break, so a poisoned
/// lock is still safe to use.
fn lock<T>(state: &Mutex<T>) -> MutexGuard<'_, T> {
    state.lock().unwrap_or_else(|e| e.into_inner())
}
