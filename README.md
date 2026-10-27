# nanokontrol2

Rust driver for the Korg nanoKONTROL2. It provides typed control events, LED control, and
snapshots of the current control state.

`NanoKontrol2::connect` puts the device in Korg's native mode, in which every control sends a
fixed message and every LED follows the computer. It works regardless of the current mode or
settings, needs no setup, and leaves the stored settings unchanged.

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

## Leaving native mode

Dropping the connection returns the device to its normal mode. A process that ends without
dropping it, such as on Ctrl-C, leaves the device in native mode until it is unplugged, so
other apps continue to receive native-mode messages. Use `exit_handle` in a Ctrl-C handler:

```rust,ignore
let exit = nk.exit_handle();
ctrlc::set_handler(move || {
    exit.leave_native();
    std::process::exit(0);
})?;
```

Run `cargo run --example monitor` to print events and toggle button LEDs.

TRACK and MARKER buttons have no LEDs. Faders and knobs report 128 positions.

## Features

The default `midir` feature provides device discovery and connections. Without it, the crate
provides `decode`, `led_message`, and the `sysex` module for use with another MIDI backend.

On Linux, `midir` requires `libasound2-dev` or `alsa-lib`.

## Licence

MIT or Apache-2.0.
