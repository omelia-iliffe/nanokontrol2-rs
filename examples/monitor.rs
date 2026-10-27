//! Prints every event from the device and toggles a button's LED on each press.

use nanokontrol2::{Event, NanoKontrol2};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (mut nk, events) = NanoKontrol2::connect()?;

    // The default Ctrl-C action skips Drop, so leave native mode explicitly.
    let exit = nk.exit_handle();
    ctrlc::set_handler(move || {
        exit.leave_native();
        std::process::exit(0);
    })?;

    nk.set_all_leds(false)?;
    println!("connected, Ctrl-C to quit");

    for event in events {
        match &event {
            Event::Unmapped(bytes) => println!("unmapped {bytes:02X?}"),
            Event::Button {
                button,
                pressed: true,
            } => {
                let lit = !nk.state().led(*button);
                nk.set_led(*button, lit)?;
                println!("{event:?} -> LED {}", if lit { "on" } else { "off" });
            }
            _ => println!("{event:?}"),
        }
    }
    Ok(())
}
