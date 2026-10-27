//! Shows or changes the device's scene settings.
//!
//! `cargo run --example scene` prints them.
//! `cargo run --example scene -- led external` (or `internal`) sets the LED mode, and
//! `-- mode cc` (or `cubase`, `dp`, `live`, `protools`, `sonar`) the control mode. Changes last
//! until the device is unplugged; add `save` to keep them.

use nanokontrol2::NanoKontrol2;
use nanokontrol2::sysex::{ControlMode, LedMode};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let save = args.last() == Some(&"save");

    let (mut nk, _events) = NanoKontrol2::connect_unchecked(NanoKontrol2::PORT_NAME)?;
    match args.as_slice() {
        [] => {}
        ["led", mode, ..] => nk.set_led_mode(led_mode(mode)?, save)?,
        ["mode", mode, ..] => nk.set_control_mode(control_mode(mode)?, save)?,
        _ => return Err("expected `led <mode>` or `mode <mode>`".into()),
    }
    let (channel, scene) = nk.read_scene()?;
    println!("global channel {}: {scene:?}", channel + 1);
    Ok(())
}

fn led_mode(name: &str) -> Result<LedMode, String> {
    match name {
        "external" => Ok(LedMode::External),
        "internal" => Ok(LedMode::Internal),
        _ => Err(format!("unknown LED mode {name:?}")),
    }
}

fn control_mode(name: &str) -> Result<ControlMode, String> {
    match name {
        "cc" => Ok(ControlMode::Cc),
        "cubase" => Ok(ControlMode::Cubase),
        "dp" => Ok(ControlMode::DigitalPerformer),
        "live" => Ok(ControlMode::Live),
        "protools" => Ok(ControlMode::ProTools),
        "sonar" => Ok(ControlMode::Sonar),
        _ => Err(format!("unknown control mode {name:?}")),
    }
}
