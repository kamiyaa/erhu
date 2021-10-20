use std::path::PathBuf;
use std::time;

use serde_derive::{Deserialize, Serialize};

use crate::error::{DiziError, DiziErrorKind, DiziResult};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum ClientRequest {
    // quit
    #[serde(rename = "/client/leave")]
    Leave { uuid: String },
    // quit server
    #[serde(rename = "/server/quit")]
    ServerQuit,

    // player requests
    #[serde(rename = "/player/state")]
    PlayerState,
    #[serde(rename = "/player/play/file")]
    PlayerFilePlay { path: PathBuf },

    #[serde(rename = "/player/play/next")]
    PlayerPlayNext,
    #[serde(rename = "/player/play/previous")]
    PlayerPlayPrevious,

    #[serde(rename = "/player/pause")]
    PlayerPause,
    #[serde(rename = "/player/resume")]
    PlayerResume,
    #[serde(rename = "/player/volume/get")]
    PlayerGetVolume,

    #[serde(rename = "/player/rewind")]
    PlayerRewind { amount: time::Duration },
    #[serde(rename = "/player/fast_forward")]
    PlayerFastForward { amount: time::Duration  },

    #[serde(rename = "/player/toggle/play")]
    PlayerTogglePlay,
    #[serde(rename = "/player/toggle/next")]
    PlayerToggleNext,
    #[serde(rename = "/player/toggle/repeat")]
    PlayerToggleRepeat,
    #[serde(rename = "/player/toggle/shuffle")]
    PlayerToggleShuffle,

    #[serde(rename = "/player/volume/increase")]
    PlayerVolumeUp { amount: usize },
    #[serde(rename = "/player/volume/decrease")]
    PlayerVolumeDown { amount: usize },

    // playlist requests
    #[serde(rename = "/playlist/state")]
    PlaylistState,
    #[serde(rename = "/playlist/open")]
    PlaylistOpen { path: PathBuf },
    #[serde(rename = "/playlist/play")]
    PlaylistPlay { index: usize },

    #[serde(rename = "/playlist/append")]
    PlaylistAppend { path: PathBuf },
    #[serde(rename = "/playlist/remove")]
    PlaylistRemove { index: usize },
    #[serde(rename = "/playlist/move_up")]
    PlaylistMoveUp { index: usize },
    #[serde(rename = "/playlist/move_down")]
    PlaylistMoveDown { index: usize },
}

impl ClientRequest {
    pub fn api_path(&self) -> &'static str {
        match &*self {
            Self::Leave { .. } => "/client/leave",
            Self::ServerQuit => "/server/quit",
            Self::PlayerState => "/player/state",
            Self::PlayerFilePlay { .. } => "/player/play/file",
            Self::PlayerPlayNext => "/player/play/next",
            Self::PlayerPlayPrevious => "/player/play/previous",
            Self::PlayerPause => "/player/pause",
            Self::PlayerResume => "/player/resume",
            Self::PlayerGetVolume => "/player/volume/get",
            Self::PlayerRewind { .. } => "/player/rewind",
            Self::PlayerFastForward { .. } => "/player/fast_forward",
            Self::PlayerTogglePlay => "/player/toggle/play",
            Self::PlayerToggleNext => "/player/toggle/next",
            Self::PlayerToggleRepeat => "/player/toggle/repeat",
            Self::PlayerToggleShuffle => "/player/toggle/shuffle",
            Self::PlayerVolumeUp { .. } => "/player/volume/increase",
            Self::PlayerVolumeDown { .. } => "/player/volume/decrease",
            Self::PlaylistState => "/playlist/state",
            Self::PlaylistOpen { .. } => "/playlist/open",
            Self::PlaylistPlay { .. } => "/playlist/play",
            Self::PlaylistAppend { .. } => "/playlist/append",
            Self::PlaylistRemove { .. } => "/playlist/remove",
            Self::PlaylistMoveUp { .. } => "/playlist/move_up",
            Self::PlaylistMoveDown { .. } => "/playlist/move_down"
        }
    }

    pub fn parse_str(s: &str, args: &str) -> DiziResult<Self> {
        match s {
            "/client/leave" => Ok(Self::Leave { uuid: "".to_string() } ),

            "/server/quit" => Ok(Self::ServerQuit),

            "/player/state" => Ok(Self::PlayerState),
            "/player/play/file" => Ok(Self::PlayerFilePlay { path: PathBuf::new() }),

            "/player/play/next" => Ok(Self::PlayerPlayNext),
            "/player/play/previous" => Ok(Self::PlayerPlayPrevious),

            "/player/pause" => Ok(Self::PlayerPause),
            "/player/resume" => Ok(Self::PlayerResume),
            "/player/volume/get" => Ok(Self::PlayerGetVolume),

            "/player/rewind" => Ok(Self::PlayerRewind { amount: time::Duration::from_secs(1) }),
            "/player/fast_forward" => Ok(Self::PlayerFastForward { amount: time::Duration::from_secs(1) }),

            "/player/toggle/play" => Ok(Self::PlayerTogglePlay),
            "/player/toggle/next" => Ok(Self::PlayerToggleNext),
            "/player/toggle/repeat" => Ok(Self::PlayerToggleRepeat),
            "/player/toggle/shuffle" => Ok(Self::PlayerToggleShuffle),

            "/player/volume/increase" => Ok(Self::PlayerVolumeUp { amount: 1 }),
            "/player/volume/decrease" => Ok(Self::PlayerVolumeDown { amount: 1 }),

            "/playlist/state" => Ok(Self::PlaylistState),
            "/playlist/open" => Ok(Self::PlaylistOpen { path: PathBuf::new() }),
            "/playlist/play" => Ok(Self::PlaylistPlay { index: 0 }),

            "/playlist/append" => Ok(Self::PlaylistAppend { path: PathBuf::new() }),
            "/playlist/remove" => Ok(Self::PlaylistRemove { index: 0 }),
            "/playlist/move_up" => Ok(Self::PlaylistMoveUp { index: 0 }),
            "/playlist/move_down" => Ok(Self::PlaylistMoveDown { index: 0 }),

            s => Err(DiziError::new(DiziErrorKind::UnrecognizedCommand, format!("Unrecognized command: '{}'", s))),
        }
    }
}
