# nanokontrol2

Rust driver for the Korg nanoKONTROL2 in CC mode. It provides typed control events, LED
control, and snapshots of the current control state.

`NanoKontrol2::connect` reads the device's scene over SysEx and uses its control assignments,
including changes made in Korg Kontrol Editor.

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

`nk.state()` returns current fader, knob, button, and LED states.

## Device setup

The device must use CC mode. Computer-controlled LEDs also require LED Mode External.

Use the `scene` example to inspect or save these settings. Run it without arguments to show
the current settings:

```sh
cargo run --example scene
cargo run --example scene -- mode cc save
cargo run --example scene -- led external save
```

In code, connect with `connect_unchecked` before changing the control mode. Without `save`,
changes last until the device is unplugged.

Run `cargo run --example monitor` to print events and toggle button LEDs.

TRACK and MARKER buttons have no LEDs. Faders and knobs report 128 positions.

## Features

The default `midir` feature provides device discovery and connections. Without it, the crate
provides MIDI decoding, LED messages, and SysEx helpers for another MIDI backend.

On Linux, `midir` requires `libasound2-dev` or `alsa-lib`.

## Licence

MIT or Apache-2.0.
