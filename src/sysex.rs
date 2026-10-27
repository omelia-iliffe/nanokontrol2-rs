//! Reading and writing the device's scene over SysEx, per Korg's "nanoKONTROL2 MIDI
//! Implementation" (revision 1.00).
//!
//! A scene write takes effect immediately but only lasts until the device is unplugged unless
//! it is followed by [`scene_write_request`], which saves it to the device's memory.

use std::fmt;

const KORG: u8 = 0x42;
const FAMILY: [u8; 4] = [0x00, 0x01, 0x13, 0x00];

const DUMP_REQUEST: u8 = 0x1F;
const DUMP: u8 = 0x7F;
const REPLY: u8 = 0x5F;

const FUNC_SCENE_DUMP_REQUEST: u8 = 0x10;
const FUNC_SCENE_WRITE_REQUEST: u8 = 0x11;
const FUNC_SCENE_DUMP: u8 = 0x40;
const FUNC_WRITE_COMPLETED: u8 = 0x21;
const FUNC_WRITE_ERROR: u8 = 0x22;
const FUNC_LOAD_COMPLETED: u8 = 0x23;
const FUNC_LOAD_ERROR: u8 = 0x24;

/// Length of a scene's parameters (TABLE 1).
pub const SCENE_LEN: usize = 339;
/// Length of a scene once packed into 7-bit MIDI bytes.
const PACKED_LEN: usize = 388;
/// "Num of Data" in a scene dump: the function ID byte plus the packed scene.
const DUMP_DATA_LEN: usize = 1 + PACKED_LEN;

const LED_MODE: usize = 2;
const CONTROL_MODE: usize = 1;
const GLOBAL_CHANNEL: usize = 0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LedMode {
    /// The device lights each button's LED itself and ignores LED messages.
    Internal,
    /// LEDs only change when the computer sends the button's message.
    External,
}

/// The device's operating mode, as stored in the scene. The power-up button combinations set
/// the same value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlMode {
    Cc,
    Cubase,
    DigitalPerformer,
    Live,
    ProTools,
    Sonar,
}

const CONTROL_MODES: [ControlMode; 6] = [
    ControlMode::Cc,
    ControlMode::Cubase,
    ControlMode::DigitalPerformer,
    ControlMode::Live,
    ControlMode::ProTools,
    ControlMode::Sonar,
];

/// The device's scene: every setting Korg Kontrol Editor can change.
#[derive(Clone, PartialEq, Eq)]
pub struct Scene(pub [u8; SCENE_LEN]);

impl fmt::Debug for Scene {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Scene")
            .field("global_channel", &self.global_channel())
            .field("control_mode", &self.control_mode())
            .field("led_mode", &self.led_mode())
            .finish_non_exhaustive()
    }
}

impl Scene {
    /// 0 to 15.
    pub fn global_channel(&self) -> u8 {
        self.0[GLOBAL_CHANNEL]
    }

    /// `None` for a value outside the documented range.
    pub fn control_mode(&self) -> Option<ControlMode> {
        CONTROL_MODES
            .get(usize::from(self.0[CONTROL_MODE]))
            .copied()
    }

    pub fn set_control_mode(&mut self, mode: ControlMode) {
        self.0[CONTROL_MODE] = mode as u8;
    }

    pub fn led_mode(&self) -> LedMode {
        if self.0[LED_MODE] == 0 {
            LedMode::Internal
        } else {
            LedMode::External
        }
    }

    pub fn set_led_mode(&mut self, mode: LedMode) {
        self.0[LED_MODE] = mode as u8;
    }
}

/// A SysEx message from the device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reply {
    /// Answer to [`search_device_request`], carrying the device's global MIDI channel.
    SearchDevice {
        global_channel: u8,
    },
    Scene(Box<Scene>),
    /// The device accepted a scene dump.
    LoadCompleted,
    LoadError,
    /// The device saved its scene to memory.
    WriteCompleted,
    WriteError,
}

fn header(global_channel: u8) -> Vec<u8> {
    let mut msg = vec![0xF0, KORG, 0x40 | (global_channel & 0x0F)];
    msg.extend_from_slice(&FAMILY);
    msg
}

fn two_byte_request(global_channel: u8, function: u8) -> Vec<u8> {
    let mut msg = header(global_channel);
    msg.extend_from_slice(&[DUMP_REQUEST, function, 0x00, 0xF7]);
    msg
}

/// Asks any connected nanoKONTROL2 for its global MIDI channel, which every other message here
/// must be addressed with. `echo` is copied into the reply.
pub fn search_device_request(echo: u8) -> Vec<u8> {
    vec![0xF0, KORG, 0x50, 0x00, echo & 0x7F, 0xF7]
}

pub fn scene_dump_request(global_channel: u8) -> Vec<u8> {
    two_byte_request(global_channel, FUNC_SCENE_DUMP_REQUEST)
}

/// Saves the device's current scene to its memory, so it survives being unplugged.
pub fn scene_write_request(global_channel: u8) -> Vec<u8> {
    two_byte_request(global_channel, FUNC_SCENE_WRITE_REQUEST)
}

/// Replaces the device's current scene.
pub fn scene_dump(global_channel: u8, scene: &Scene) -> Vec<u8> {
    let mut msg = header(global_channel);
    let len = DUMP_DATA_LEN as u16;
    msg.extend_from_slice(&[DUMP, 0x7F, 0x02, (len >> 7) as u8, (len & 0x7F) as u8]);
    msg.push(FUNC_SCENE_DUMP);
    msg.extend(pack(&scene.0));
    msg.push(0xF7);
    msg
}

/// Returns `None` for anything that is not a recognised nanoKONTROL2 reply.
pub fn parse_reply(msg: &[u8]) -> Option<Reply> {
    if let [0xF0, KORG, 0x50, 0x01, g, _echo, 0x13, 0x01, 0x00, 0x00, ..] = *msg {
        return Some(Reply::SearchDevice {
            global_channel: g & 0x0F,
        });
    }
    let [0xF0, KORG, g, rest @ ..] = msg else {
        return None;
    };
    if g & 0xF0 != 0x40 {
        return None;
    }
    let body = rest.strip_prefix(&FAMILY)?.strip_suffix(&[0xF7])?;
    match *body {
        [REPLY, FUNC_LOAD_COMPLETED, 0x00] => Some(Reply::LoadCompleted),
        [REPLY, FUNC_LOAD_ERROR, 0x00] => Some(Reply::LoadError),
        [REPLY, FUNC_WRITE_COMPLETED, 0x00] => Some(Reply::WriteCompleted),
        [REPLY, FUNC_WRITE_ERROR, 0x00] => Some(Reply::WriteError),
        [
            DUMP,
            0x7F,
            0x02,
            len_msb,
            len_lsb,
            FUNC_SCENE_DUMP,
            ref packed @ ..,
        ] => {
            let len = usize::from(len_msb) << 7 | usize::from(len_lsb);
            if len != DUMP_DATA_LEN || packed.len() != PACKED_LEN {
                return None;
            }
            Some(Reply::Scene(Box::new(Scene(unpack(packed)?))))
        }
        _ => None,
    }
}

/// Korg's 8-bit to 7-bit packing (NOTE 2): each group of up to 7 bytes is preceded by a byte
/// holding their top bits, bit `i` for byte `i`.
fn pack(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() / 7 * 8 + 8);
    for chunk in data.chunks(7) {
        let msbs = chunk
            .iter()
            .enumerate()
            .fold(0, |acc, (i, b)| acc | (b >> 7) << i);
        out.push(msbs);
        out.extend(chunk.iter().map(|b| b & 0x7F));
    }
    out
}

fn unpack(packed: &[u8]) -> Option<[u8; SCENE_LEN]> {
    let mut out = [0; SCENE_LEN];
    let mut n = 0;
    for chunk in packed.chunks(8) {
        let (&msbs, bytes) = chunk.split_first()?;
        for (i, b) in bytes.iter().enumerate() {
            *out.get_mut(n)? = b | ((msbs >> i) & 1) << 7;
            n += 1;
        }
    }
    (n == SCENE_LEN).then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_scene() -> Scene {
        let mut data = [0; SCENE_LEN];
        for (i, b) in data.iter_mut().enumerate() {
            *b = (i * 37) as u8;
        }
        Scene(data)
    }

    #[test]
    fn packing_round_trips_with_documented_length() {
        let scene = sample_scene();
        let packed = pack(&scene.0);
        assert_eq!(packed.len(), PACKED_LEN);
        assert!(packed.iter().all(|b| b & 0x80 == 0));
        assert_eq!(unpack(&packed), Some(scene.0));
    }

    #[test]
    fn scene_dump_parses_back() {
        let scene = sample_scene();
        let msg = scene_dump(3, &scene);
        assert_eq!(
            msg[..13],
            [
                0xF0, 0x42, 0x43, 0x00, 0x01, 0x13, 0x00, 0x7F, 0x7F, 0x02, 0x03, 0x05, 0x40
            ]
        );
        assert_eq!(parse_reply(&msg), Some(Reply::Scene(Box::new(scene))));
    }

    #[test]
    fn requests_match_spec() {
        assert_eq!(
            scene_dump_request(0),
            [
                0xF0, 0x42, 0x40, 0x00, 0x01, 0x13, 0x00, 0x1F, 0x10, 0x00, 0xF7
            ]
        );
        assert_eq!(
            scene_write_request(0),
            [
                0xF0, 0x42, 0x40, 0x00, 0x01, 0x13, 0x00, 0x1F, 0x11, 0x00, 0xF7
            ]
        );
    }

    #[test]
    fn parses_acks() {
        let ack = [
            0xF0, 0x42, 0x40, 0x00, 0x01, 0x13, 0x00, 0x5F, 0x23, 0x00, 0xF7,
        ];
        assert_eq!(parse_reply(&ack), Some(Reply::LoadCompleted));
        let search = [
            0xF0, 0x42, 0x50, 0x01, 0x02, 0x11, 0x13, 0x01, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00,
            0xF7,
        ];
        assert_eq!(
            parse_reply(&search),
            Some(Reply::SearchDevice { global_channel: 2 })
        );
    }

    #[test]
    fn led_mode_byte() {
        let mut scene = Scene([0; SCENE_LEN]);
        assert_eq!(scene.led_mode(), LedMode::Internal);
        scene.set_led_mode(LedMode::External);
        assert_eq!(scene.0[2], 1);
        assert_eq!(scene.control_mode(), Some(ControlMode::Cc));
    }
}
