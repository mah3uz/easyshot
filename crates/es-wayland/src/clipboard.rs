use crate::Error;
use crate::worker::State;
use es_model::platform::ClipItem;
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use rustix::fs::OFlags;
use rustix::io::Errno;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::sync::Arc;
use wayland_client::backend::ObjectId;
use wayland_client::protocol::wl_seat::WlSeat;
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle, event_created_child};
use wayland_protocols::ext::data_control::v1::client::{
    ext_data_control_device_v1::{self, ExtDataControlDeviceV1},
    ext_data_control_manager_v1::ExtDataControlManagerV1,
    ext_data_control_offer_v1::ExtDataControlOfferV1,
    ext_data_control_source_v1::{self, ExtDataControlSourceV1},
};
use wayland_protocols_wlr::data_control::v1::client::{
    zwlr_data_control_device_v1::{self, ZwlrDataControlDeviceV1},
    zwlr_data_control_manager_v1::ZwlrDataControlManagerV1,
    zwlr_data_control_offer_v1::ZwlrDataControlOfferV1,
    zwlr_data_control_source_v1::{self, ZwlrDataControlSourceV1},
};

pub(crate) type Offers = Vec<(&'static str, Arc<[u8]>)>;

const TEXT_MIMES: [&str; 5] = [
    "text/plain;charset=utf-8",
    "text/plain",
    "UTF8_STRING",
    "TEXT",
    "STRING",
];

const STALLED: Timespec = Timespec {
    tv_sec: 10,
    tv_nsec: 0,
};

pub(crate) fn offers(items: &[ClipItem]) -> Result<Offers, Error> {
    let mut out: Offers = Vec::new();
    let mut add = |mime: &'static str, bytes: Arc<[u8]>| {
        if out.iter().all(|(m, _)| *m != mime) {
            out.push((mime, bytes));
        }
    };
    for item in items {
        match item {
            ClipItem::Png(png) => add("image/png", png.clone()),
            ClipItem::UriList(paths) => {
                let mut list = String::new();
                for p in paths {
                    list.push_str(&file_uri(p)?);
                    list.push_str("\r\n");
                }
                add("text/uri-list", list.into_bytes().into());
            }
            ClipItem::GnomeCopiedFiles(paths) => {
                let mut list = String::from("copy");
                for p in paths {
                    list.push('\n');
                    list.push_str(&file_uri(p)?);
                }
                add("x-special/gnome-copied-files", list.into_bytes().into());
            }
            ClipItem::Text(text) => {
                let bytes: Arc<[u8]> = text.as_bytes().into();
                for mime in TEXT_MIMES {
                    add(mime, bytes.clone());
                }
            }
        }
    }
    Ok(out)
}

fn file_uri(path: &Path) -> Result<String, Error> {
    if !path.is_absolute() {
        return Err(Error::NotAbsolute(path.to_owned()));
    }
    let mut uri = String::from("file://");
    for &b in path.as_os_str().as_bytes() {
        if b.is_ascii_alphanumeric() || b"/-._~".contains(&b) {
            uri.push(char::from(b));
        } else {
            uri.push_str(&format!("%{b:02X}"));
        }
    }
    Ok(uri)
}

#[derive(Default)]
pub(crate) struct Clipboard {
    pub ext: Option<ExtDataControlManagerV1>,
    pub wlr: Option<ZwlrDataControlManagerV1>,
    device: Option<Device>,
}

enum Device {
    Ext(ExtDataControlDeviceV1),
    Wlr(ZwlrDataControlDeviceV1),
}

impl Device {
    fn id(&self) -> ObjectId {
        match self {
            Device::Ext(d) => d.id(),
            Device::Wlr(d) => d.id(),
        }
    }
}

impl Clipboard {
    fn ensure_device(&mut self, seat: &WlSeat, qh: &QueueHandle<State>) {
        if self.device.is_none() {
            self.device = match (&self.ext, &self.wlr) {
                (Some(m), _) => Some(Device::Ext(m.get_data_device(seat, qh, ()))),
                (None, Some(m)) => Some(Device::Wlr(m.get_data_device(seat, qh, ()))),
                (None, None) => None,
            };
        }
    }

    #[cfg(test)]
    pub(crate) fn disable_ext(&mut self) {
        self.ext = None;
        if let Some(Device::Ext(d)) = self.device.take() {
            d.destroy();
        }
    }
}

macro_rules! select {
    ($device:expr, $manager:expr, $offers:expr, $qh:expr) => {{
        let source = (!$offers.is_empty()).then(|| {
            let s = $manager.create_data_source($qh, $offers.clone());
            for (mime, _) in $offers.iter() {
                s.offer((*mime).to_owned());
            }
            s
        });
        $device.set_selection(source.as_ref());
    }};
}

impl State {
    pub(crate) fn set_clipboard(
        &mut self,
        offers: Offers,
        qh: &QueueHandle<State>,
    ) -> Result<(), Error> {
        let seat = self.seat.clone().ok_or(Error::Unsupported("wl_seat"))?;
        let offers = Arc::new(offers);
        let cb = &mut self.clipboard;
        cb.ensure_device(&seat, qh);
        match (&cb.device, &cb.ext, &cb.wlr) {
            (Some(Device::Ext(d)), Some(m), _) => select!(d, m, offers, qh),
            (Some(Device::Wlr(d)), _, Some(m)) => select!(d, m, offers, qh),
            _ => return Err(Error::Unsupported("data-control")),
        }
        tracing::debug!(mimes = ?offers.iter().map(|(m, _)| *m).collect::<Vec<_>>(), "clipboard set");
        Ok(())
    }

    fn device_finished(&mut self, id: ObjectId) {
        if self.clipboard.device.as_ref().map(Device::id) == Some(id) {
            self.clipboard.device = None;
        }
    }
}

fn serve(fd: OwnedFd, bytes: Arc<[u8]>) {
    let spawned = std::thread::Builder::new()
        .name("es-clipboard-send".into())
        .spawn(move || {
            if let Err(e) = write_all(fd.as_fd(), &bytes) {
                tracing::debug!(error = %e, "clipboard transfer ended early");
            }
        });
    if let Err(e) = spawned {
        tracing::warn!(error = %e, "cannot start a clipboard transfer");
    }
}

fn write_all(fd: BorrowedFd<'_>, mut bytes: &[u8]) -> Result<(), Error> {
    rustix::fs::fcntl_setfl(fd, rustix::fs::fcntl_getfl(fd)? | OFlags::NONBLOCK)?;
    while !bytes.is_empty() {
        let mut fds = [PollFd::new(&fd, PollFlags::OUT)];
        match poll(&mut fds, Some(&STALLED)) {
            Ok(0) => return Err(Error::Timeout),
            Ok(_) | Err(Errno::INTR) => {}
            Err(e) => return Err(e.into()),
        }
        match rustix::io::write(fd, bytes) {
            Ok(n) => bytes = &bytes[n..],
            Err(Errno::AGAIN | Errno::INTR) => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}

macro_rules! data_control {
    ($device_mod:ident, $device:ty, $source_mod:ident, $source:ty, $offer:ty) => {
        impl Dispatch<$device, ()> for State {
            fn event(
                state: &mut Self,
                device: &$device,
                event: $device_mod::Event,
                _: &(),
                _: &Connection,
                _: &QueueHandle<Self>,
            ) {
                match event {
                    $device_mod::Event::Selection { id: Some(offer) }
                    | $device_mod::Event::PrimarySelection { id: Some(offer) } => offer.destroy(),
                    $device_mod::Event::Finished => {
                        device.destroy();
                        state.device_finished(device.id());
                    }
                    _ => {}
                }
            }

            event_created_child!(State, $device, [
                $device_mod::EVT_DATA_OFFER_OPCODE => ($offer, ()),
            ]);
        }

        impl Dispatch<$source, Arc<Offers>> for State {
            fn event(
                _: &mut Self,
                source: &$source,
                event: $source_mod::Event,
                offers: &Arc<Offers>,
                _: &Connection,
                _: &QueueHandle<Self>,
            ) {
                match event {
                    $source_mod::Event::Send { mime_type, fd } => {
                        if let Some((mime, bytes)) = offers.iter().find(|(m, _)| *m == mime_type) {
                            tracing::debug!(mime, len = bytes.len(), "clipboard request");
                            serve(fd, bytes.clone());
                        }
                    }
                    $source_mod::Event::Cancelled => source.destroy(),
                    _ => {}
                }
            }
        }
    };
}

data_control!(
    ext_data_control_device_v1,
    ExtDataControlDeviceV1,
    ext_data_control_source_v1,
    ExtDataControlSourceV1,
    ExtDataControlOfferV1
);
data_control!(
    zwlr_data_control_device_v1,
    ZwlrDataControlDeviceV1,
    zwlr_data_control_source_v1,
    ZwlrDataControlSourceV1,
    ZwlrDataControlOfferV1
);

crate::capture::no_events!(
    ExtDataControlManagerV1,
    ExtDataControlOfferV1,
    ZwlrDataControlManagerV1,
    ZwlrDataControlOfferV1
);

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;
    use std::path::PathBuf;

    fn get<'a>(offers: &'a Offers, mime: &str) -> &'a [u8] {
        &offers
            .iter()
            .find(|(m, _)| *m == mime)
            .unwrap_or_else(|| panic!("{mime} offered"))
            .1
    }

    #[test]
    fn every_item_is_offered_under_each_of_its_types() {
        let png: Arc<[u8]> = b"\x89PNG".as_slice().into();
        let offers =
            offers(&[ClipItem::Png(png.clone()), ClipItem::Text("héllo".into())]).expect("offers");
        let mimes: Vec<_> = offers.iter().map(|(m, _)| *m).collect();
        assert_eq!(
            mimes,
            [
                "image/png",
                "text/plain;charset=utf-8",
                "text/plain",
                "UTF8_STRING",
                "TEXT",
                "STRING"
            ]
        );
        assert_eq!(get(&offers, "image/png"), &*png);
        assert_eq!(get(&offers, "UTF8_STRING"), "héllo".as_bytes());
    }

    #[test]
    fn file_lists_are_percent_encoded_uris_in_both_formats() {
        let paths = vec![
            PathBuf::from("/tmp/a b%.png"),
            PathBuf::from(OsStr::from_bytes(b"/x/\xff\xc3\xa9")),
        ];
        let offers = offers(&[
            ClipItem::UriList(paths.clone()),
            ClipItem::GnomeCopiedFiles(paths),
        ])
        .expect("offers");
        assert_eq!(
            get(&offers, "text/uri-list"),
            b"file:///tmp/a%20b%25.png\r\nfile:///x/%FF%C3%A9\r\n"
        );
        assert_eq!(
            get(&offers, "x-special/gnome-copied-files"),
            b"copy\nfile:///tmp/a%20b%25.png\nfile:///x/%FF%C3%A9"
        );
    }

    #[test]
    fn the_first_item_of_a_type_wins() {
        let offers =
            offers(&[ClipItem::Text("one".into()), ClipItem::Text("two".into())]).expect("offers");
        assert_eq!(offers.len(), TEXT_MIMES.len());
        assert_eq!(get(&offers, "text/plain"), b"one");
    }

    #[test]
    fn relative_paths_are_refused_because_a_uri_needs_an_absolute_one() {
        let err = offers(&[ClipItem::UriList(vec!["shot.png".into()])]);
        assert!(matches!(err, Err(Error::NotAbsolute(p)) if p == Path::new("shot.png")));
    }

    #[test]
    fn a_large_payload_reaches_a_slow_reader_whole() {
        let (read, write) = std::io::pipe().expect("pipe");
        let payload: Vec<u8> = (0..1_000_000u32).map(|i| (i % 251) as u8).collect();
        let reader = std::thread::spawn(move || {
            let mut out = Vec::new();
            let mut file = read;
            std::thread::sleep(std::time::Duration::from_millis(50));
            std::io::Read::read_to_end(&mut file, &mut out).expect("read");
            out
        });
        write_all(write.as_fd(), &payload).expect("write");
        drop(write);
        assert!(reader.join().expect("reader") == payload);
    }

    #[test]
    fn a_reader_that_goes_away_ends_the_transfer_with_an_error() {
        let (read, write) = std::io::pipe().expect("pipe");
        drop(read);
        assert!(write_all(write.as_fd(), &[0; 200_000]).is_err());
    }
}
