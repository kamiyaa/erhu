use std::fs;
use std::os::unix::net::UnixListener;
use std::path::Path;
use std::sync::mpsc;
use std::thread;

use uuid::Uuid;

use dizi_lib::error::DiziResult;
use dizi_lib::response::server::ServerBroadcastEvent;

use crate::client;
use crate::config::AppConfig;
use crate::context::{AppContext, QuitType};
use crate::events::{AppEvent, ServerEvent, ServerEventSender};
use crate::server_command::{process_client_request, send_latest_song_info};
use crate::server_commands::player::*;

pub fn setup_socket(config: &AppConfig) -> DiziResult<UnixListener> {
    let socket = Path::new(config.server_ref().socket.as_path());
    if socket.exists() {
        fs::remove_file(&socket)?;
    }
    let stream = UnixListener::bind(&socket)?;
    Ok(stream)
}

pub fn serve(config: AppConfig) -> DiziResult<()> {
    let mut context = AppContext::new(config);

    let listener = setup_socket(context.config_ref())?;
    {
        // thread for listening to new client connections
        let server_tx2 = context.events.server_event_sender().clone();
        thread::spawn(|| listen_for_clients(listener, server_tx2));
    }

    while context.quit == QuitType::DoNot {
        let event = match context.events.next() {
            Ok(event) => event,
            Err(_) => return Ok(()),
        };

        match event {
            AppEvent::Client(event) => {
                let res = process_client_request(&mut context, event);
                if let Err(err) = res {
                    eprintln!("Error: {:?}", err);
                    context
                        .events
                        .broadcast_event(ServerBroadcastEvent::ServerError {
                            msg: err.to_string(),
                        });
                }
            }
            AppEvent::Server(event) => {
                let res = process_server_event(&mut context, event);
                if let Err(err) = res {
                    eprintln!("Error: {:?}", err);
                }
            }
        }
    }

    let playlist_path = context.config_ref().server_ref().playlist_ref();
    let playlist = context.player_context_ref().player_ref().playlist_ref();

    println!("Saving playlist to '{}'", playlist_path.to_string_lossy());
    let mut file = std::fs::File::create(playlist_path)?;
    let mut writer = m3u::Writer::new(&mut file);
    for song in playlist.list_ref() {
        let entry = m3u::Entry::Path(song.file_path().to_path_buf());
        writer.write_entry(&entry)?;
    }
    println!("Playlist saved!");

    Ok(())
}

pub fn process_server_event(context: &mut AppContext, event: ServerEvent) -> DiziResult<()> {
    match event {
        ServerEvent::NewClient(stream) => {
            let client_tx2 = context.events.client_request_sender().clone();
            let (server_tx, server_rx) = mpsc::channel();

            let client_uuid = Uuid::new_v4();
            let uuid_string = client_uuid.to_string();
            thread::spawn(move || {
                client::handle_client(client_uuid, stream, client_tx2, server_rx)
            });
            context
                .events
                .add_broadcast_listener(uuid_string, server_tx);
        }
        ServerEvent::PlayerProgressUpdate(elapsed) => {
            context
                .player_context_mut()
                .player_mut()
                .set_elapsed(elapsed);
            context
                .events
                .broadcast_event(ServerBroadcastEvent::PlayerProgressUpdate { elapsed });
        }
        ServerEvent::PlayerDone => {
            process_done_song(context)?;
        }
    }
    Ok(())
}

pub fn listen_for_clients(listener: UnixListener, event_tx: ServerEventSender) -> DiziResult<()> {
    for stream in listener.incoming().flatten() {
        event_tx.send(ServerEvent::NewClient(stream));
    }
    Ok(())
}

pub fn process_done_song(context: &mut AppContext) -> DiziResult<()> {
    let next_enabled = context.player_context_ref().player_ref().next_enabled();
    let repeat_enabled = context.player_context_ref().player_ref().repeat_enabled();

    if !next_enabled {
        if repeat_enabled {
            player_play_again(context)?;
            send_latest_song_info(context)?;
        } else {
            eprintln!("Done playing song!");
        }
    } else {
        let len1 = context
            .player_context_ref()
            .player_ref()
            .dirlist_playlist_ref()
            .len();

        let len2 = context
            .player_context_ref()
            .player_ref()
            .playlist_ref()
            .len();

        let len = if len1 < len2 { len2 } else { len1 };
        for i in (1..len) {
            if player_play_next(context, i).is_err() {
                continue;
            }
            send_latest_song_info(context)?;
            break;
        }
    }
    Ok(())
}
