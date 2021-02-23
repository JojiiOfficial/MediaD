use human_panic::setup_panic;
use std::path::Path;

use epoll::{Event, Events};
use lazy_static::lazy_static;
use mpris::{PlaybackStatus, Player, PlayerFinder};
use pulsectl::controllers::{DeviceControl, SinkController};
use std::sync::Mutex;

#[derive(Default)]
struct Data {
    last_active_player: Option<String>,
}

lazy_static! {
    static ref GLOBAL_DATA: Mutex<Data> = Mutex::new(Data::default());
}

fn main() {
    setup_panic!();

    if std::env::args().count() < 2 {
        println!(
            "Usage: {} <device-id>",
            std::env::args()
                .next()
                .unwrap_or_else(|| "mediad".to_owned())
        );

        return;
    }

    let device_path = std::env::args().nth(1).unwrap();
    let mut device =
        evdev::Device::open(&Path::new("/dev/input/by-id").join(&device_path)).unwrap();
    let mut state = KeyState::default();

    // Request epoll FD
    let epoll_fd = epoll::create(true).expect("Couldn't open epoll FD. Update your kernel!");

    // Add device's fd to epoll's FD
    epoll::ctl(
        epoll_fd,
        epoll::ControlOptions::EPOLL_CTL_ADD,
        device.fd(),
        Event::new(Events::EPOLLIN | Events::EPOLLET, 0),
    )
    .expect("Couldn't add devices fd to epoll");

    // Epoll buffer
    let mut events = [Event::new(Events::empty(), 0); 1];

    loop {
        // Wait for epoll events
        epoll::wait(epoll_fd, -1, &mut events).expect("epoll wait failed");

        // Handle all new events
        for ev in device.events_no_sync().unwrap() {
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
        x if x == evdev::KEY_MUTE as u16 => mute_action(),
        x if x == evdev::KEY_VOLUMEUP as u16 => volume_action(0.05),
        x if x == evdev::KEY_VOLUMEDOWN as u16 => volume_action(-0.05),
        x if x == evdev::KEY_NEXTSONG as u16 => run_mpris_action(MprisAction::NextSong),
        x if x == evdev::KEY_PREVIOUSSONG as u16 => run_mpris_action(MprisAction::PreviousSong),
        x if x == evdev::KEY_PLAYPAUSE as u16 => run_mpris_action(MprisAction::PlayPause),
        x if x == evdev::KEY_STOPCD as u16 => run_mpris_action(MprisAction::Stop),
        _ => Ok(()),
    }
}

#[derive(PartialEq)]
enum MprisAction {
    PlayPause,
    Stop,
    NextSong,
    PreviousSong,
}

/// Run Mpris command
fn run_mpris_action(action: MprisAction) -> Result<(), String> {
    let found_players = PlayerFinder::new()
        .map_err(|_| "Player finder can't created".to_string())?
        .find_all()
        .map_err(|_| "can't find all players".to_string())?;

    let player = {
        if found_players.is_empty() {
            return Err("No player found!".to_string());
        } else if found_players.len() == 1 {
            found_players
                .first()
                .ok_or_else(|| "no first player".to_string())?
        } else {
            // Get all stopped players
            let stopped_players = get_players_by_state(&found_players, PlaybackStatus::Stopped);
            let paused_players = get_players_by_state(&found_players, PlaybackStatus::Paused);
            let playing_players = get_players_by_state(&found_players, PlaybackStatus::Playing);

            // Prefer playing players
            match playing_players
                .get(0)
                .map(|i| Some(i.to_owned()))
                // Try to use paused player next
                .unwrap_or_else(|| {
                    if paused_players.is_empty() {
                        if stopped_players.len() == 1 {
                            Some(stopped_players.get(0).unwrap().to_owned())
                        } else {
                            None
                        }
                    } else if paused_players.len() == 1 {
                        Some(paused_players.get(0).unwrap().to_owned())
                    } else {
                        None
                    }
                })
                .to_owned()
            {
                Some(e) => e,
                None => {
                    let alternative = found_players
                        .first()
                        .ok_or_else(|| "no first player".to_string())?;

                    let global_data_lock = GLOBAL_DATA.lock().unwrap();
                    if global_data_lock.last_active_player.is_some() {
                        found_players
                            .iter()
                            .find(|i| {
                                i.unique_name()
                                    == global_data_lock.last_active_player.as_ref().unwrap()
                            })
                            .unwrap_or(alternative)
                    } else {
                        alternative
                    }
                }
            }
        }
    };

    if (action == MprisAction::PreviousSong || action == MprisAction::NextSong)
        && player
            .get_playback_status()
            .map_err(|_| "Can't get playback status".to_string())?
            == PlaybackStatus::Stopped
    {}

    GLOBAL_DATA.lock().unwrap().last_active_player = Some(player.unique_name().to_owned());

    match action {
        MprisAction::Stop => player.stop(),
        MprisAction::PlayPause => player.play_pause(),
        MprisAction::NextSong => player.next(),
        MprisAction::PreviousSong => player.previous(),
    }
    .map_err(|i| i.to_string())
}

fn get_players_by_state<'a, 'b>(
    players: &'b Vec<Player<'a>>,
    state: PlaybackStatus,
) -> Vec<&'b Player<'a>> {
    players
        .iter()
        .filter(|i| i.get_playback_status().unwrap_or(PlaybackStatus::Stopped) == state)
        .collect()
}

/// Set default device's volume
fn volume_action(delta: f64) -> Result<(), String> {
    let mut handler = SinkController::create()
        .map_err(|_| "controller error".to_owned())
        .expect("Can't open connection to pulse audio");

    let device = handler
        .get_default_device()
        .map_err(|_| "controller error".to_owned())?;

    // Entmute first
    if device.mute {
        mute_action()?;
    }

    if delta < 0 as f64 {
        handler.decrease_device_volume_by_percent(device.index, delta * -1_f64);
    } else {
        handler.increase_device_volume_by_percent(device.index, delta);
    }

    Ok(())
}

/// Mute current default device
fn mute_action() -> Result<(), String> {
    let mut handler = SinkController::create()
        .map_err(|_| "controller error".to_owned())
        .expect("Can't open connection to pulse audio");

    let device = handler
        .get_default_device()
        .map_err(|_| "controller error".to_owned())?;

    handler.set_device_mute_by_index(device.index, !device.mute);

    Ok(())
}
