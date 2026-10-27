# nanokontrol2

A Rust driver for the Korg nanoKONTROL2 in CC mode: typed events for every fader, knob and
button, LED control, and a snapshot of the current control state.

On connect, the crate reads the device's scene over SysEx and decodes with its assignments,
so scenes edited in Korg Kontrol Editor work without extra configuration.

## Usage

```rust,no_run
use nanokontrol2::{Event, NanoKontrol2};

fn main() -> Result<(), nanokontrol2::Error> {
    let (mut nk, events) = NanoKontrol2::connect()?;
    for event in events {
        match event {
            Event::Fader { strip, value } => println!("fader {strip}: {value:.2}"),
            Event::Button { button, pressed: true } => {
                let lit = nk.state().led(button);
                nk.set_led(button, !lit)?;
            }
            _ => {}
        }
    }
    Ok(())
}
```

`nk.state()` returns fader and knob positions, held buttons and LED states at any time.

## Device setup

- **CC mode.** `connect` fails with `Error::NotCcMode` if the device is in one of its DAW
  modes. Plug it in while holding SET MARKER and CYCLE, or switch it with
  `set_control_mode(ControlMode::Cc, true)` after `connect_unchecked`
  (`ControlMode` and `LedMode` are in `nanokontrol2::sysex`).
- **LED Mode External.** At the factory setting the device lights its own LEDs and ignores
  `set_led`. Switch once with `set_led_mode(LedMode::External, true)`; the setting is saved on
  the device.

The `scene` example does both from the command line:

```sh
cargo run --example scene                         # show the current settings
cargo run --example scene -- led external save
cargo run --example scene -- mode cc save
```

`cargo run --example monitor` prints every event and toggles LEDs on button presses.

The TRACK and MARKER buttons have no LEDs. Faders and knobs report 128 positions.

## Features

- `midir` (default): device discovery and connection through [midir]. Without it, the crate
  only decodes and encodes MIDI bytes (`Map::decode`, `Map::led_message`, and the `sysex`
  module), for use with any MIDI backend.

On Linux, `midir` needs the ALSA development package (`libasound2-dev` or `alsa-lib`).

## Licence

MIT or Apache-2.0, at your option.

[midir]: https://crates.io/crates/midir
