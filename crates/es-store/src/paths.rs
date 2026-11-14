use crate::atomic::ensure_private_dir;
use crate::error::{Error, Result};
use directories::{ProjectDirs, UserDirs};
use es_model::Config;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Paths {
    config: PathBuf,
    data: PathBuf,
    state: PathBuf,
    cache: PathBuf,
    videos: PathBuf,
}

impl Paths {
    pub fn from_xdg() -> Result<Self> {
        let dirs = ProjectDirs::from("", "", "easyshot").ok_or_else(|| Error::Io {
            path: PathBuf::from("~"),
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "no home directory"),
        })?;
        Ok(Self {
            config: dirs.config_dir().to_path_buf(),
            data: dirs.data_dir().to_path_buf(),
            state: dirs
                .state_dir()
                .unwrap_or_else(|| dirs.data_local_dir())
                .to_path_buf(),
            cache: dirs.cache_dir().to_path_buf(),
            videos: default_videos_dir().unwrap_or_else(|| dirs.data_dir().join("videos")),
        })
    }

    pub fn rooted(root: &Path) -> Self {
        Self {
            config: root.join("config"),
            data: root.join("data"),
            state: root.join("state"),
            cache: root.join("cache"),
            videos: root.join("videos"),
        }
    }

    pub fn config_dir(&self) -> &Path {
        &self.config
    }

    pub fn config_file(&self) -> PathBuf {
        self.config.join("config.toml")
    }

    /// The compositor binds file easyshot writes; `name` comes from the bind helper.
    pub fn binds_file(&self, name: &str) -> PathBuf {
        self.config.join(name)
    }

    pub fn style_file(&self) -> PathBuf {
        self.config.join("annotation-style.json")
    }

    pub fn bg_presets_file(&self) -> PathBuf {
        self.config.join("background-presets.json")
    }

    pub fn bg_presets_recovery_file(&self) -> PathBuf {
        self.config.join("background-presets.json.recovery")
    }

    /// In the data folder, not config: config folders are often kept in a shared dotfiles repository.
    pub fn bucket_keys_file(&self) -> PathBuf {
        self.data.join("bucket-keys.json")
    }

    pub fn shares_file(&self) -> PathBuf {
        self.data.join("shares.json")
    }

    pub fn share_install_token_file(&self) -> PathBuf {
        self.state.join("share-install-token")
    }

    pub fn gradients_file(&self) -> PathBuf {
        self.config.join("gradients.json")
    }

    pub fn gradients_recovery_file(&self) -> PathBuf {
        self.config.join("gradients.json.recovery")
    }

    pub fn library_dir(&self) -> PathBuf {
        self.data.join("library")
    }

    pub fn wallpapers_dir(&self) -> PathBuf {
        self.data.join("wallpapers")
    }

    pub fn history_index(&self) -> PathBuf {
        self.library_dir().join("index.json")
    }

    pub fn deck_dir(&self) -> PathBuf {
        self.data.join("deck")
    }

    pub fn recordings_dir(&self) -> PathBuf {
        self.data.join("recordings")
    }

    /// Finished recordings and video exports; project data stays in `recordings_dir`.
    pub fn videos_dir(&self) -> PathBuf {
        self.videos.clone()
    }

    pub fn recovery_dir(&self) -> PathBuf {
        self.state.join("recovery")
    }

    pub fn state_file(&self) -> PathBuf {
        self.state.join("state.json")
    }

    pub fn agent_file(&self) -> PathBuf {
        self.state.join("agent.json")
    }

    pub fn log_dir(&self) -> PathBuf {
        self.state.join("log")
    }

    pub fn crash_dir(&self) -> PathBuf {
        self.state.join("crash")
    }

    pub fn thumbs_dir(&self) -> PathBuf {
        self.cache.join("thumbs")
    }

    pub fn clipboard_dir(&self) -> PathBuf {
        self.cache.join("clipboard")
    }

    pub fn drag_dir(&self) -> PathBuf {
        self.cache.join("drag")
    }

    pub fn unstaged_dir(&self) -> PathBuf {
        self.cache.join("unstaged")
    }

    pub fn share_dir(&self) -> PathBuf {
        self.cache.join("share")
    }

    pub fn video_copies_dir(&self) -> PathBuf {
        self.cache.join("video-copies")
    }

    pub fn agent_dir(&self) -> PathBuf {
        self.cache.join("agent")
    }

    pub fn ensure_all(&self) -> Result<()> {
        [
            self.config_dir().to_path_buf(),
            self.library_dir(),
            self.deck_dir(),
            self.recordings_dir(),
            self.recovery_dir(),
            self.log_dir(),
            self.crash_dir(),
            self.thumbs_dir(),
            self.clipboard_dir(),
            self.drag_dir(),
            self.unstaged_dir(),
            self.share_dir(),
            self.video_copies_dir(),
            self.agent_dir(),
        ]
        .iter()
        .try_for_each(|d| ensure_private_dir(d))
    }
}

pub fn default_save_dir() -> Option<PathBuf> {
    let dirs = UserDirs::new()?;
    let pictures = dirs
        .picture_dir()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| dirs.home_dir().join("Pictures"));
    Some(pictures.join("Screenshots"))
}

fn default_videos_dir() -> Option<PathBuf> {
    let dirs = UserDirs::new()?;
    let videos = dirs
        .video_dir()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| dirs.home_dir().join("Videos"));
    Some(videos.join("easyshot"))
}

pub fn save_dir(config: &Config) -> Option<PathBuf> {
    config.general.save_dir.clone().or_else(default_save_dir)
}
