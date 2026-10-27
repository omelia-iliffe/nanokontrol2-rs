//! The SysEx messages that switch the device in and out of native mode, per Korg's
//! "nanoKONTROL2 MIDI Implementation" (revision 1.00).
//!
//! In native mode every control sends a fixed CC on channel 16 and every LED follows the
//! computer, regardless of the stored settings. Native mode remains active until the device
//! receives an exit request or is unplugged.

const KORG: u8 = 0x42;
const FAMILY: [u8; 4] = [0x00, 0x01, 0x13, 0x00];

const NATIVE_REQUEST: u8 = 0x00;
const NATIVE_REPLY: u8 = 0x40;
const DUMP_REQUEST: u8 = 0x1F;
const REPLY: u8 = 0x5F;

const FUNC_MODE_REQUEST: u8 = 0x12;
const FUNC_MODE_DATA: u8 = 0x42;
const FUNC_NAK: u8 = 0x24;

/// A SysEx message from the device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reply {
    /// Answer to [`search_device_request`], carrying the device's global MIDI channel.
    SearchDevice { global_channel: u8 },
    /// Reports whether native mode is active.
    NativeMode(bool),
    /// The device rejected a request.
    Nak,
}

fn header(global_channel: u8) -> Vec<u8> {
    let mut msg = vec![0xF0, KORG, 0x40 | (global_channel & 0x0F)];
    msg.extend_from_slice(&FAMILY);
    msg
}

/// Asks any connected nanoKONTROL2 for the global MIDI channel used by other requests. `echo`
/// is copied into the reply.
pub fn search_device_request(echo: u8) -> Vec<u8> {
    vec![0xF0, KORG, 0x50, 0x00, echo & 0x7F, 0xF7]
}

/// Enters (`true`) or leaves native mode.
pub fn native_mode_request(global_channel: u8, native: bool) -> Vec<u8> {
    let mut msg = header(global_channel);
    msg.extend_from_slice(&[NATIVE_REQUEST, 0x00, u8::from(native), 0xF7]);
    msg
}

/// Asks whether the device is in native mode.
pub fn mode_request(global_channel: u8) -> Vec<u8> {
    let mut msg = header(global_channel);
    msg.extend_from_slice(&[DUMP_REQUEST, FUNC_MODE_REQUEST, 0x00, 0xF7]);
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
    match *rest.strip_prefix(&FAMILY)?.strip_suffix(&[0xF7])? {
        [REPLY, FUNC_MODE_DATA, mode] => Some(Reply::NativeMode(mode == 0x01)),
        [REPLY, FUNC_NAK, 0x00] => Some(Reply::Nak),
        [NATIVE_REPLY, 0x00, 0x02] => Some(Reply::NativeMode(false)),
        [NATIVE_REPLY, 0x00, 0x03] => Some(Reply::NativeMode(true)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_match_spec() {
        assert_eq!(
            native_mode_request(0, true),
            [
                0xF0, 0x42, 0x40, 0x00, 0x01, 0x13, 0x00, 0x00, 0x00, 0x01, 0xF7
            ]
        );
        assert_eq!(
            mode_request(2),
            [
                0xF0, 0x42, 0x42, 0x00, 0x01, 0x13, 0x00, 0x1F, 0x12, 0x00, 0xF7
            ]
        );
        assert_eq!(
            search_device_request(5),
            [0xF0, 0x42, 0x50, 0x00, 0x05, 0xF7]
        );
    }

    #[test]
    fn parses_replies() {
        let reply = |body: [u8; 3]| {
            let mut msg = vec![0xF0, 0x42, 0x40, 0x00, 0x01, 0x13, 0x00];
            msg.extend_from_slice(&body);
            msg.push(0xF7);
            parse_reply(&msg)
        };
        assert_eq!(reply([0x40, 0x00, 0x03]), Some(Reply::NativeMode(true)));
        assert_eq!(reply([0x40, 0x00, 0x02]), Some(Reply::NativeMode(false)));
        assert_eq!(reply([0x5F, 0x42, 0x01]), Some(Reply::NativeMode(true)));
        assert_eq!(reply([0x5F, 0x24, 0x00]), Some(Reply::Nak));
        let search = [
            0xF0, 0x42, 0x50, 0x01, 0x02, 0x11, 0x13, 0x01, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00,
            0xF7,
        ];
        assert_eq!(
            parse_reply(&search),
            Some(Reply::SearchDevice { global_channel: 2 })
        );
    }
}
