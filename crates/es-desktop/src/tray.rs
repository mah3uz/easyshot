use crate::bus::{Labels, block, connect, protocol};
use crate::dbus::Endpoint;
use crate::{Error, RecordingTrayState, Result, Tray, TrayItem, TrayModel};
use es_protocol::Route;
use ksni::menu::{MenuItem, StandardItem, SubMenu};
use ksni::{Icon, TrayMethods};

const WATCHER: &str = "org.kde.StatusNotifierWatcher";

pub async fn watcher_present(endpoint: &Endpoint) -> bool {
    let Ok(conn) = connect(endpoint).await else {
        return false;
    };
    let Ok(dbus) = zbus::fdo::DBusProxy::new(&conn).await else {
        return false;
    };
    let Ok(name) = zbus::names::BusName::try_from(WATCHER) else {
        return false;
    };
    dbus.name_has_owner(name).await.unwrap_or(false)
}

#[derive(Debug, Clone)]
pub struct IconPixmap {
    pub width: i32,
    pub height: i32,
    pub argb: Vec<u8>,
}

struct EsTray {
    items: Vec<TrayItem>,
    recording: Option<RecordingTrayState>,
    labels: Labels,
    pixmaps: Vec<IconPixmap>,
    tx: async_channel::Sender<Route>,
}

fn menu_item(item: &TrayItem, labels: &Labels) -> MenuItem<EsTray> {
    if item.separator {
        return MenuItem::Separator;
    }
    let label = item.label.clone().unwrap_or_else(|| labels(item.label_id));
    let shortcut: Vec<Vec<String>> = item
        .shortcut
        .iter()
        .map(|s| s.split('+').map(str::to_owned).collect())
        .collect();
    if !item.children.is_empty() {
        return SubMenu {
            label,
            enabled: item.enabled,
            submenu: item.children.iter().map(|c| menu_item(c, labels)).collect(),
            ..Default::default()
        }
        .into();
    }
    let route = item.route.clone();
    StandardItem {
        label,
        enabled: item.enabled,
        shortcut,
        activate: Box::new(move |t: &mut EsTray| {
            if let Some(route) = &route {
                let _ = t.tx.try_send(route.clone());
            }
        }),
        ..Default::default()
    }
    .into()
}

fn clock(state: &RecordingTrayState) -> String {
    let s = state.elapsed.as_secs();
    format!("{:02}:{:02}", s / 60, s % 60)
}

impl ksni::Tray for EsTray {
    fn id(&self) -> String {
        "dev.easyshot.Easyshot".into()
    }

    fn title(&self) -> String {
        match &self.recording {
            None => "Easyshot".into(),
            Some(s) if s.paused => format!("Easyshot {} paused", clock(s)),
            Some(s) => format!("Easyshot {}", clock(s)),
        }
    }

    fn icon_name(&self) -> String {
        "dev.easyshot.Easyshot-symbolic".into()
    }

    fn icon_pixmap(&self) -> Vec<Icon> {
        self.pixmaps
            .iter()
            .map(|p| Icon {
                width: p.width,
                height: p.height,
                data: p.argb.clone(),
            })
            .collect()
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        let _ = self.tx.try_send(Route::CaptureBar);
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        self.items
            .iter()
            .map(|i| menu_item(i, &self.labels))
            .collect()
    }
}

enum Command {
    Menu(TrayModel),
    Recording(Option<RecordingTrayState>),
}

pub struct KsniTray {
    commands: async_channel::Sender<Command>,
    activations: async_channel::Receiver<Route>,
}

impl KsniTray {
    // ksni connects to the process session bus itself; a missing watcher
    // surfaces as Unsupported.
    pub async fn start(labels: Labels, pixmaps: Vec<IconPixmap>) -> Result<Self> {
        let (tx, activations) = async_channel::unbounded();
        let handle = EsTray {
            items: Vec::new(),
            recording: None,
            labels,
            pixmaps,
            tx,
        }
        .spawn()
        .await
        .map_err(|e| match e {
            ksni::Error::WontShow => Error::Unsupported,
            ksni::Error::Watcher(zbus::fdo::Error::ServiceUnknown(_)) => Error::Unsupported,
            other => protocol(other),
        })?;
        let (commands, queue) = async_channel::unbounded();
        std::thread::Builder::new()
            .name("es-tray".into())
            .spawn(move || {
                while let Ok(command) = queue.recv_blocking() {
                    block(handle.update(|t| match command {
                        Command::Menu(model) => t.items = model.items,
                        Command::Recording(state) => t.recording = state,
                    }));
                }
            })
            .map_err(|source| Error::Io {
                path: "es-tray".into(),
                source,
            })?;
        Ok(KsniTray {
            commands,
            activations,
        })
    }
}

impl Tray for KsniTray {
    fn set_menu(&self, m: TrayModel) {
        let _ = self.commands.try_send(Command::Menu(m));
    }

    fn set_recording(&self, s: Option<RecordingTrayState>) {
        let _ = self.commands.try_send(Command::Recording(s));
    }

    fn activations(&self) -> async_channel::Receiver<Route> {
        self.activations.clone()
    }
}
