use std::path::Path;

use mpris::PlayerFinder;

fn main() {
    let device_path = std::env::args().nth(1).unwrap().to_owned();
    let mut d = evdev::Device::open(&Path::new("/dev/input/by-id").join(&device_path)).unwrap();
    let mut state = KeyState::default();

    loop {
        for ev in d.events_no_sync().unwrap() {
            if ev._type != 1 {
                continue;
            }

            if let Err(err) = run_key_event(ev.code, ev.value, &mut state) {
                println!("An error occured: {}", err)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct KeyState {
    pub id: u16,
    pub state: bool,
}

fn run_key_event(code: u16, value: i32, shared_state: &mut KeyState) -> Result<(), String> {
    // Ignore repeating events if already value == 1
    if shared_state.id == code && shared_state.state && value == 1 {
        return Ok(());
    }

    // Set keyState to pressed
    if shared_state.id == code && shared_state.state && value == 0 {
        shared_state.state = false;
        return Ok(());
    }

    // Set new shared state
    shared_state.id = code;
    shared_state.state = value == 1;

    // Execute action
    match code {
        /*
        x if x == evdev::KEY_VOLUMEUP as u16 => println!("vol up"),
        x if x == evdev::KEY_VOLUMEDOWN as u16 => println!("vol down"),
        x if x == evdev::KEY_MUTE as u16 => println!("vol down"),
        */
        x if x == evdev::KEY_NEXTSONG as u16 => run_mpris_action(MprisAction::NextSong),
        x if x == evdev::KEY_PREVIOUSSONG as u16 => run_mpris_action(MprisAction::PreviousSong),
        x if x == evdev::KEY_PLAYPAUSE as u16 => run_mpris_action(MprisAction::PlayPause),
        x if x == evdev::KEY_STOPCD as u16 => run_mpris_action(MprisAction::Stop),
        _ => Ok(()),
    }
}

enum MprisAction {
    PlayPause,
    Stop,
    NextSong,
    PreviousSong,
}

/// Run Mpris command
fn run_mpris_action(action: MprisAction) -> Result<(), String> {
    let player = PlayerFinder::new()
        .expect("Could not connect to D-Bus")
        .find_active()
        .expect("Could not find any player");

    match action {
        MprisAction::Stop => player.stop(),
        MprisAction::PlayPause => player.play_pause(),
        MprisAction::NextSong => player.next(),
        MprisAction::PreviousSong => player.previous(),
    }
    .map_err(|i| i.to_string())
}
