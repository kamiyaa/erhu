use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use std::time;

use rand::seq::SliceRandom;
use rand::thread_rng;

use dizi_lib::error::{DiziError, DiziErrorKind, DiziResult};
use dizi_lib::player::{PlayerState, PlayerStatus};
use dizi_lib::playlist::{DirlistPlaylist, Playlist, PlaylistStatus};
use dizi_lib::song::Song;

use crate::audio::{player_stream, PlayerRequest};
use crate::config;
use crate::events::ServerEventSender;

pub fn read_playlist(cwd: &Path, path: &Path) -> io::Result<Playlist> {
    let mut reader = m3u::Reader::open(path)?;
    let read_playlist: Vec<_> = reader.entries().map(|entry| entry.unwrap()).collect();
    let mut playlist = Playlist::new();
    for entry in &read_playlist {
        match entry {
            m3u::Entry::Path(p) => {
                if p.is_absolute() {
                    if let Ok(song) = Song::new(&p) {
                        playlist.append_song(song);
                    }
                } else {
                    let mut new_path = cwd.to_path_buf();
                    new_path.push(p);
                    if let Ok(song) = Song::new(&new_path) {
                        playlist.append_song(song);
                    }
                }
            }
            _ => {}
        }
    }
    Ok(playlist)
}

#[derive(Debug)]
pub struct Player {
    current_song: Option<Song>,
    elapsed: time::Duration,

    status: PlayerStatus,
    _playlist_status: PlaylistStatus,

    volume: f32,

    shuffle: bool,
    repeat: bool,
    next: bool,

    playlist: Playlist,

    dirlist_playlist: DirlistPlaylist,

    event_tx: ServerEventSender,

    player_handle: thread::JoinHandle<DiziResult<()>>,
    player_req_tx: mpsc::Sender<PlayerRequest>,
    player_res_rx: mpsc::Receiver<DiziResult<()>>,
    // event_tx: mpsc::Sender<PlayerResponse>,
}

impl Player {
    pub fn new(config_t: &config::AppConfig, event_tx: ServerEventSender) -> Self {
        let (player_req_tx, player_req_rx) = mpsc::channel();
        let (player_res_tx, player_res_rx) = mpsc::channel();

        let config_t2 = config_t.clone();
        let event_tx2 = event_tx.clone();
        let player_handle = thread::spawn(move || {
            let res = player_stream(config_t2, player_res_tx, player_req_rx, event_tx2);
            match res.as_ref() {
                Ok(_) => {}
                Err(e) => eprintln!("PlayerStream: {:?}", e),
            }
            res
        });

        let server_config = config_t.server_ref();
        let player_config = server_config.player_ref();

        let playlist = read_playlist(&PathBuf::from("/"), server_config.playlist_ref())
            .unwrap_or_else(|_| Playlist::new());

        Self {
            current_song: None,
            elapsed: time::Duration::from_secs(0),

            status: PlayerStatus::Stopped,
            _playlist_status: PlaylistStatus::PlaylistFile,
            volume: 0.5,

            shuffle: player_config.shuffle,
            repeat: player_config.repeat,
            next: player_config.next,

            event_tx,

            playlist,
            dirlist_playlist: DirlistPlaylist::new(),
            player_handle,
            player_req_tx,
            player_res_rx,
        }
    }

    pub fn clone_player_state(&self) -> PlayerState {
        let song = self.current_song_ref().map(|s| s.clone());
        let elapsed = self.get_elapsed();
        let status = self.play_status();
        let playlist_status = self.playlist_status();
        let volume: usize = (self.get_volume() * 100.0) as usize;
        let shuffle = self.shuffle_enabled();
        let next = self.next_enabled();
        let repeat = self.repeat_enabled();

        let playlist = self.playlist_ref().clone();

        PlayerState {
            song,
            elapsed,

            status,
            playlist_status,

            volume,

            next,
            repeat,
            shuffle,

            playlist,
        }
    }

    fn player_stream_req(&self) -> &mpsc::Sender<PlayerRequest> {
        &self.player_req_tx
    }
    fn player_stream_res(&self) -> &mpsc::Receiver<DiziResult<()>> {
        &self.player_res_rx
    }

    pub fn play(&mut self, song: &Song) -> DiziResult<()> {
        self.player_stream_req()
            .send(PlayerRequest::Play(song.clone()))?;
        let _resp = self.player_stream_res().recv()??;

        self.status = PlayerStatus::Playing;
        self.current_song = Some(song.clone());
        Ok(())
    }

    pub fn play_entire_directory(&mut self, path: &Path) -> DiziResult<()> {
        let song = Song::new(path)?;

        let dirlist_playlist = match song.file_path().parent() {
            Some(parent) => {
                // make the playlist and make sure the first song is the current song
                let mut playlist = DirlistPlaylist::from(parent)?;
                // sort alphabetically or randomly if shuffle is enabled
                if !self.shuffle_enabled() {
                    playlist.list_mut().sort();
                } else {
                    playlist.list_mut().shuffle(&mut thread_rng());
                }

                let index = playlist
                    .list_mut()
                    .iter()
                    .enumerate()
                    .find(|(_, p)| p.as_path() == path)
                    .map(|(i, _)| i);
                if let Some(index) = index {
                    playlist.index = index;
                }
                playlist
            }
            None => DirlistPlaylist::new(),
        };

        self.play(&song)?;
        self.dirlist_playlist = dirlist_playlist;
        self._playlist_status = PlaylistStatus::DirectoryListing;
        Ok(())
    }

    pub fn play_from_directory(&mut self, index: usize) -> DiziResult<()> {
        if index >= self.dirlist_playlist_ref().len() {
            return Err(DiziError::new(
                DiziErrorKind::InvalidParameters,
                "index out of bounds".to_string(),
            ));
        }
        let path = self.dirlist_playlist_ref().list_ref()[index].clone();
        let song = Song::new(path.as_path())?;
        self.play(&song)?;
        self.dirlist_playlist_mut().set_playing_index(index);
        self._playlist_status = PlaylistStatus::DirectoryListing;
        Ok(())
    }

    pub fn play_from_playlist(&mut self, index: usize) -> DiziResult<()> {
        if index >= self.playlist.len() {
            return Err(DiziError::new(
                DiziErrorKind::InvalidParameters,
                "index out of bounds".to_string(),
            ));
        }
        let song = self.playlist.list_ref()[index].clone();
        self.play(&song)?;
        self.playlist_mut().set_playing_index(Some(index));
        self._playlist_status = PlaylistStatus::PlaylistFile;
        Ok(())
    }

    pub fn pause(&mut self) -> DiziResult<()> {
        self.player_stream_req().send(PlayerRequest::Pause)?;

        self.player_stream_res().recv()??;
        self.status = PlayerStatus::Paused;
        Ok(())
    }

    pub fn resume(&mut self) -> DiziResult<()> {
        self.player_stream_req().send(PlayerRequest::Resume)?;

        self.player_stream_res().recv()??;
        self.status = PlayerStatus::Playing;
        Ok(())
    }

    pub fn toggle_play(&mut self) -> DiziResult<PlayerStatus> {
        match self.status {
            PlayerStatus::Playing => {
                self.pause()?;
                Ok(PlayerStatus::Paused)
            }
            PlayerStatus::Paused => {
                self.resume()?;
                Ok(PlayerStatus::Playing)
            }
            _ => Ok(PlayerStatus::Stopped),
        }
    }
    pub fn play_status(&self) -> PlayerStatus {
        self.status
    }
    pub fn playlist_status(&self) -> PlaylistStatus {
        self._playlist_status
    }
    pub fn set_playlist_status(&mut self, status: PlaylistStatus) {
        self._playlist_status = status;
    }

    pub fn get_volume(&self) -> f32 {
        self.volume
    }
    pub fn set_volume(&mut self, volume: f32) -> DiziResult<()> {
        self.player_stream_req()
            .send(PlayerRequest::SetVolume(volume))?;

        self.player_stream_res().recv()??;
        self.volume = volume;
        Ok(())
    }
    pub fn next_enabled(&self) -> bool {
        self.next
    }
    pub fn repeat_enabled(&self) -> bool {
        self.repeat
    }
    pub fn shuffle_enabled(&self) -> bool {
        self.shuffle
    }

    pub fn set_next(&mut self, next: bool) {
        self.next = next;
    }
    pub fn set_repeat(&mut self, repeat: bool) {
        self.repeat = repeat;
    }
    pub fn set_shuffle(&mut self, shuffle: bool) {
        self.shuffle = shuffle;
        if self.shuffle_enabled() {
            self.playlist.list_mut().shuffle(&mut thread_rng());
            self.dirlist_playlist.list_mut().shuffle(&mut thread_rng());
        }
    }

    pub fn get_elapsed(&self) -> time::Duration {
        self.elapsed
    }
    pub fn set_elapsed(&mut self, elapsed: time::Duration) {
        self.elapsed = elapsed;
    }

    pub fn current_song_ref(&self) -> Option<&Song> {
        self.current_song.as_ref()
    }

    pub fn playlist_ref(&self) -> &Playlist {
        &self.playlist
    }
    pub fn playlist_mut(&mut self) -> &mut Playlist {
        &mut self.playlist
    }

    pub fn dirlist_playlist_ref(&self) -> &DirlistPlaylist {
        &self.dirlist_playlist
    }
    pub fn dirlist_playlist_mut(&mut self) -> &mut DirlistPlaylist {
        &mut self.dirlist_playlist
    }
}
