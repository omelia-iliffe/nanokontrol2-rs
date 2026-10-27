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
    let (args, save) = match args.split_last() {
        Some((&"save", rest)) => (rest, true),
        _ => (&args[..], false),
    };

    let (mut nk, _events) = NanoKontrol2::connect_unchecked(NanoKontrol2::PORT_NAME)?;
    match args {
        [] => {}
        ["led", mode] => nk.set_led_mode(led_mode(mode)?, save)?,
        ["mode", mode] => nk.set_control_mode(control_mode(mode)?, save)?,
        _ => {
            eprintln!("{USAGE}");
            std::process::exit(2);
        }
    }
    let (channel, scene) = nk.read_scene()?;
    let mode = scene
        .control_mode()
        .map_or("unknown".to_string(), |m| m.to_string());
    println!(
        "channel {}, mode {mode}, LED {}",
        channel + 1,
        scene.led_mode()
    );
    Ok(())
}

const USAGE: &str =
    "usage: scene [led external|internal | mode cc|cubase|dp|live|protools|sonar] [save]";

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
