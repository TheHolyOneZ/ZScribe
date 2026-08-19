use crate::hotkey::Hotkey;
use ashpd::desktop::global_shortcuts::{GlobalShortcuts, NewShortcut};
use futures_util::StreamExt;
use thiserror::Error;

const SHORTCUT_ID: &str = "toggle-recording";
const DESCRIPTION: &str = "Start or stop recording with ZScribe";

#[derive(Debug, Error)]
pub enum PortalError {
    #[error("the desktop portal is unavailable: {0}")]
    Unavailable(String),
    #[error("the compositor refused to bind a global shortcut")]
    Refused,
}

#[derive(Debug, Clone)]
pub struct BoundShortcut {
    pub trigger: String,
}

pub async fn run(
    hotkey: &Hotkey,
    on_bound: impl FnOnce(BoundShortcut) + Send + 'static,
    on_activate: impl Fn() + Send + 'static,
) -> Result<(), PortalError> {
    let shortcuts = GlobalShortcuts::new()
        .await
        .map_err(|err| PortalError::Unavailable(err.to_string()))?;

    let session = shortcuts
        .create_session()
        .await
        .map_err(|err| PortalError::Unavailable(err.to_string()))?;

    let mut activations = shortcuts
        .receive_activated()
        .await
        .map_err(|err| PortalError::Unavailable(err.to_string()))?;

    let request = shortcuts
        .bind_shortcuts(
            &session,
            &[NewShortcut::new(SHORTCUT_ID, DESCRIPTION)
                .preferred_trigger(Some(hotkey.to_portal_trigger().as_str()))],
            &ashpd::WindowIdentifier::default(),
        )
        .await
        .map_err(|err| PortalError::Unavailable(err.to_string()))?;

    let bound = request.response().map_err(|_| PortalError::Refused)?;

    let trigger = bound
        .shortcuts()
        .iter()
        .find(|shortcut| shortcut.id() == SHORTCUT_ID)
        .map(|shortcut| shortcut.trigger_description().to_owned())
        .ok_or(PortalError::Refused)?;

    tracing::info!(%trigger, "global shortcut bound through the desktop portal");
    on_bound(BoundShortcut { trigger });

    while let Some(activation) = activations.next().await {
        if activation.shortcut_id() == SHORTCUT_ID {
            on_activate();
        }
    }

    tracing::warn!("the desktop portal ended the global shortcut session");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shortcut_has_a_stable_id_and_a_description_the_user_will_see() {
        assert!(!SHORTCUT_ID.is_empty());
        assert!(DESCRIPTION.contains("ZScribe"));
        assert!(DESCRIPTION.contains("recording"));
    }

    #[test]
    fn errors_explain_themselves() {
        assert!(!PortalError::Refused.to_string().is_empty());
        assert!(!PortalError::Unavailable("no service".to_owned())
            .to_string()
            .is_empty());
    }
}
