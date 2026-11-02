pub trait Sound: Send + Sync {
    fn shutter(&self);
}

#[cfg(feature = "sound")]
pub use rodio_sound::RodioSound;

#[cfg(feature = "sound")]
mod rodio_sound {
    use super::Sound;
    use std::io::Cursor;

    static SHUTTER: &[u8] = include_bytes!("../../../assets/sounds/shutter.ogg");

    #[derive(Debug, Default, Clone, Copy)]
    pub struct RodioSound;

    impl Sound for RodioSound {
        fn shutter(&self) {
            // The output device is opened on a thread of its own so a slow or missing device never delays a capture.
            let spawned = std::thread::Builder::new()
                .name("es-shutter".into())
                .spawn(|| {
                    let Ok(sink) = rodio::DeviceSinkBuilder::open_default_sink() else {
                        return;
                    };
                    if let Ok(player) = rodio::play(sink.mixer(), Cursor::new(SHUTTER)) {
                        player.sleep_until_end();
                    }
                });
            if let Err(e) = spawned {
                tracing::warn!("shutter sound thread failed: {e}");
            }
        }
    }

    #[cfg(test)]
    mod tests {
        #[test]
        fn the_bundled_shutter_decodes() {
            assert!(rodio::Decoder::new(std::io::Cursor::new(super::SHUTTER)).is_ok());
        }
    }
}
