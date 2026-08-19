use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum DisplayServer {
    Windows,
    MacOs,
    X11,
    Wayland,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum HotkeyBackend {
    Os,

    Portal,

    ExternalCommand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum NoteSeverity {
    Info,

    Degraded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CapabilityNote {
    pub severity: NoteSeverity,
    pub title: String,
    pub detail: String,
    pub remedy: Option<String>,
}

impl CapabilityNote {
    pub fn info(title: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            severity: NoteSeverity::Info,
            title: title.into(),
            detail: detail.into(),
            remedy: None,
        }
    }

    pub fn degraded(title: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            severity: NoteSeverity::Degraded,
            title: title.into(),
            detail: detail.into(),
            remedy: None,
        }
    }

    pub fn with_remedy(mut self, remedy: impl Into<String>) -> Self {
        self.remedy = Some(remedy.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Capabilities {
    pub display_server: DisplayServer,
    pub hotkey: HotkeyBackend,
    pub notes: Vec<CapabilityNote>,
}

impl Capabilities {
    pub fn is_degraded(&self) -> bool {
        self.notes
            .iter()
            .any(|note| note.severity == NoteSeverity::Degraded)
    }

    pub fn detect() -> Self {
        Self::from_environment(&Environment::probe())
    }

    pub fn from_environment(env: &Environment) -> Self {
        match env.display_server {
            DisplayServer::Windows | DisplayServer::MacOs | DisplayServer::X11 => Self {
                display_server: env.display_server,
                hotkey: HotkeyBackend::Os,
                notes: Vec::new(),
            },

            DisplayServer::Wayland => {
                let mut notes = Vec::new();

                let hotkey = if env.has_global_shortcuts_portal {
                    HotkeyBackend::Portal
                } else {
                    notes.push(
                        CapabilityNote::degraded(
                            "Global hotkey unavailable",
                            "This compositor does not implement the desktop portal for global \
                             shortcuts, so ZScribe cannot register a hotkey itself. Recording \
                             from the window and the tray still works.",
                        )
                        .with_remedy(
                            "Bind a key in your compositor's own settings to the command \
                             `zscribe record`.",
                        ),
                    );
                    HotkeyBackend::ExternalCommand
                };

                Self {
                    display_server: DisplayServer::Wayland,
                    hotkey,
                    notes,
                }
            }

            DisplayServer::Unknown => Self {
                display_server: DisplayServer::Unknown,
                hotkey: HotkeyBackend::ExternalCommand,
                notes: vec![CapabilityNote::degraded(
                    "No graphical session detected",
                    "Neither X11 nor Wayland could be found, so a global hotkey cannot be \
                     registered.",
                )],
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Environment {
    pub display_server: DisplayServer,
    pub has_global_shortcuts_portal: bool,
}

impl Environment {
    pub fn probe() -> Self {
        Self {
            display_server: detect_display_server(),
            has_global_shortcuts_portal: global_shortcuts_portal_present(),
        }
    }

    #[cfg(test)]
    pub fn for_test(display_server: DisplayServer) -> Self {
        Self {
            display_server,
            has_global_shortcuts_portal: false,
        }
    }
}

fn detect_display_server() -> DisplayServer {
    if cfg!(target_os = "windows") {
        return DisplayServer::Windows;
    }
    if cfg!(target_os = "macos") {
        return DisplayServer::MacOs;
    }

    match std::env::var("XDG_SESSION_TYPE").as_deref() {
        Ok("wayland") => return DisplayServer::Wayland,
        Ok("x11") => return DisplayServer::X11,
        _ => {}
    }
    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        return DisplayServer::Wayland;
    }
    if std::env::var_os("DISPLAY").is_some() {
        return DisplayServer::X11;
    }
    DisplayServer::Unknown
}

fn global_shortcuts_portal_present() -> bool {
    if !cfg!(target_os = "linux") {
        return false;
    }

    let dirs = [
        "/usr/share/xdg-desktop-portal/portals",
        "/usr/local/share/xdg-desktop-portal/portals",
    ];

    dirs.iter().any(|dir| {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return false;
        };
        entries.filter_map(Result::ok).any(|entry| {
            std::fs::read_to_string(entry.path())
                .map(|contents| contents.contains("GlobalShortcuts"))
                .unwrap_or(false)
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_conventional_desktops_register_their_own_hotkey() {
        for server in [
            DisplayServer::Windows,
            DisplayServer::MacOs,
            DisplayServer::X11,
        ] {
            let caps = Capabilities::from_environment(&Environment::for_test(server));
            assert_eq!(caps.hotkey, HotkeyBackend::Os, "{server:?}");
            assert!(!caps.is_degraded(), "{server:?}");
        }
    }

    #[test]
    fn wayland_with_a_portal_uses_it_and_is_not_degraded() {
        let env = Environment {
            has_global_shortcuts_portal: true,
            ..Environment::for_test(DisplayServer::Wayland)
        };
        let caps = Capabilities::from_environment(&env);

        assert_eq!(caps.hotkey, HotkeyBackend::Portal);
        assert!(!caps.is_degraded());
    }

    #[test]
    fn wayland_without_a_portal_falls_back_to_an_external_binding_and_says_how() {
        let caps = Capabilities::from_environment(&Environment::for_test(DisplayServer::Wayland));

        assert_eq!(caps.hotkey, HotkeyBackend::ExternalCommand);
        assert!(caps.is_degraded());

        let note = caps
            .notes
            .iter()
            .find(|n| n.title.contains("hotkey"))
            .expect("must explain the missing hotkey");
        assert!(note
            .remedy
            .as_ref()
            .is_some_and(|r| r.contains("zscribe record")));
    }

    #[test]
    fn losing_the_hotkey_does_not_imply_losing_recording() {
        let caps = Capabilities::from_environment(&Environment::for_test(DisplayServer::Wayland));
        let note = &caps.notes[0];
        assert!(note.detail.contains("tray"), "got: {}", note.detail);
    }

    #[test]
    fn a_headless_session_is_degraded_rather_than_pretending_to_work() {
        let caps = Capabilities::from_environment(&Environment::for_test(DisplayServer::Unknown));
        assert_eq!(caps.hotkey, HotkeyBackend::ExternalCommand);
        assert!(caps.is_degraded());
    }

    #[test]
    fn every_note_has_a_title_and_a_detail() {
        for server in [
            DisplayServer::Windows,
            DisplayServer::MacOs,
            DisplayServer::X11,
            DisplayServer::Wayland,
            DisplayServer::Unknown,
        ] {
            let caps = Capabilities::from_environment(&Environment::for_test(server));
            for note in &caps.notes {
                assert!(!note.title.is_empty(), "{server:?}");
                assert!(!note.detail.is_empty(), "{server:?}");
            }
        }
    }

    #[test]
    fn a_note_builds_with_and_without_a_remedy() {
        let plain = CapabilityNote::info("t", "d");
        assert_eq!(plain.severity, NoteSeverity::Info);
        assert_eq!(plain.remedy, None);

        let fixed = CapabilityNote::degraded("t", "d").with_remedy("do this");
        assert_eq!(fixed.severity, NoteSeverity::Degraded);
        assert_eq!(fixed.remedy.as_deref(), Some("do this"));
    }

    #[test]
    fn probing_the_real_session_does_not_panic() {
        let caps = Capabilities::detect();
        assert!(caps.notes.iter().all(|n| !n.title.is_empty()));
    }
}
