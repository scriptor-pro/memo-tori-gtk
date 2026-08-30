use async_channel::Sender;
use ksni::blocking::TrayMethods;
use ksni::menu::StandardItem;
use ksni::{MenuItem, Tray};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayEvent {
    ToggleWindow,
    ShowCapture,
    ShowNotes,
    Quit,
}

/// ksni callbacks run on the tray's own DBus thread, so GTK widgets must
/// never be touched from here. Only the `Send`-safe event is handed off;
/// the receiver side (on the GTK main loop) does the actual UI work.
struct AppTray {
    sender: Sender<TrayEvent>,
}

impl Tray for AppTray {
    fn id(&self) -> String {
        "io.github.memo_tori.gtk".into()
    }

    fn icon_name(&self) -> String {
        "memo-tori".into()
    }

    fn title(&self) -> String {
        "Memo-Tori".into()
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        let _ = self.sender.send_blocking(TrayEvent::ToggleWindow);
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        vec![
            StandardItem {
                label: "Nouvelle capture".into(),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.sender.send_blocking(TrayEvent::ShowCapture);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Notes".into(),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.sender.send_blocking(TrayEvent::ShowNotes);
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Quitter".into(),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.sender.send_blocking(TrayEvent::Quit);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

/// Registers the StatusNotifierItem tray on the session bus and returns the
/// receiving end of the event channel. Poll it on the GTK main loop (e.g.
/// via `glib::spawn_future_local`) to react to tray interactions.
///
/// Returns `Err` when no StatusNotifierWatcher is available (e.g. some
/// minimal window managers); callers must treat this as "tray unsupported"
/// per agent.md's documented fallback behavior.
pub fn spawn() -> anyhow::Result<async_channel::Receiver<TrayEvent>> {
    let (sender, receiver) = async_channel::unbounded();

    AppTray { sender }
        .spawn()
        .map(|_handle| receiver)
        .map_err(|err| anyhow::anyhow!("tray registration failed: {err}"))
}
