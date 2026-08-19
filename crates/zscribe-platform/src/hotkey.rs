use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;
use ts_rs::TS;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum HotkeyError {
    #[error("enter a key combination")]
    Empty,
    #[error(
        "a global hotkey needs at least one modifier (Ctrl, Alt, Shift or Super), otherwise it \
         would start recording while you are typing"
    )]
    NoModifier,
    #[error("a global hotkey needs a key as well as modifiers")]
    NoKey,
    #[error("'{0}' is not a key ZScribe recognises")]
    UnknownKey(String),
    #[error("'{0}' appears more than once")]
    Duplicate(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Modifier {
    Ctrl,
    Alt,
    Shift,
    Super,
}

impl Modifier {
    fn accelerator_token(self) -> &'static str {
        match self {
            Modifier::Ctrl => "Ctrl",
            Modifier::Alt => "Alt",
            Modifier::Shift => "Shift",
            Modifier::Super => "Super",
        }
    }

    fn display_token(self) -> &'static str {
        match self {
            Modifier::Ctrl => {
                if cfg!(target_os = "macos") {
                    "⌃"
                } else {
                    "Ctrl"
                }
            }
            Modifier::Alt => {
                if cfg!(target_os = "macos") {
                    "⌥"
                } else {
                    "Alt"
                }
            }
            Modifier::Shift => {
                if cfg!(target_os = "macos") {
                    "⇧"
                } else {
                    "Shift"
                }
            }
            Modifier::Super => {
                if cfg!(target_os = "macos") {
                    "⌘"
                } else if cfg!(target_os = "windows") {
                    "Win"
                } else {
                    "Super"
                }
            }
        }
    }

    fn from_token(token: &str) -> Option<Self> {
        match token.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => Some(Modifier::Ctrl),
            "alt" | "option" => Some(Modifier::Alt),
            "shift" => Some(Modifier::Shift),
            "super" | "meta" | "cmd" | "command" | "win" => Some(Modifier::Super),

            "commandorcontrol" | "cmdorctrl" => Some(if cfg!(target_os = "macos") {
                Modifier::Super
            } else {
                Modifier::Ctrl
            }),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Hotkey {
    pub modifiers: Vec<Modifier>,
    pub key: String,
}

impl Hotkey {
    pub fn parse(accelerator: &str) -> Result<Self, HotkeyError> {
        if accelerator.trim().is_empty() {
            return Err(HotkeyError::Empty);
        }

        let mut modifiers: Vec<Modifier> = Vec::new();
        let mut key: Option<String> = None;

        for raw in accelerator.split('+') {
            let token = raw.trim();
            if token.is_empty() {
                continue;
            }

            if let Some(modifier) = Modifier::from_token(token) {
                if modifiers.contains(&modifier) {
                    return Err(HotkeyError::Duplicate(token.to_owned()));
                }
                modifiers.push(modifier);
            } else {
                let normalised = normalise_key(token)
                    .ok_or_else(|| HotkeyError::UnknownKey(token.to_owned()))?;
                if key.is_some() {
                    return Err(HotkeyError::Duplicate(token.to_owned()));
                }
                key = Some(normalised);
            }
        }

        let key = key.ok_or(HotkeyError::NoKey)?;
        if modifiers.is_empty() {
            return Err(HotkeyError::NoModifier);
        }

        modifiers.sort_unstable();
        Ok(Self { modifiers, key })
    }

    pub fn to_portal_trigger(&self) -> String {
        let mut parts: Vec<String> = self
            .modifiers
            .iter()
            .map(|modifier| {
                match modifier {
                    Modifier::Ctrl => "CTRL",
                    Modifier::Alt => "ALT",
                    Modifier::Shift => "SHIFT",
                    Modifier::Super => "LOGO",
                }
                .to_owned()
            })
            .collect();

        parts.push(if self.key.len() == 1 {
            self.key.to_ascii_lowercase()
        } else {
            self.key.clone()
        });

        parts.join("+")
    }

    pub fn to_accelerator(&self) -> String {
        let mut parts: Vec<&str> = self
            .modifiers
            .iter()
            .map(|m| m.accelerator_token())
            .collect();
        parts.push(&self.key);
        parts.join("+")
    }
}

impl fmt::Display for Hotkey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let separator = if cfg!(target_os = "macos") { "" } else { " + " };
        let mut parts: Vec<&str> = self.modifiers.iter().map(|m| m.display_token()).collect();
        parts.push(&self.key);
        write!(f, "{}", parts.join(separator))
    }
}

fn normalise_key(token: &str) -> Option<String> {
    let upper = token.to_ascii_uppercase();

    if upper.len() == 1 && upper.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Some(upper);
    }

    if let Some(number) = upper.strip_prefix('F') {
        if let Ok(n) = number.parse::<u8>() {
            if (1..=24).contains(&n) {
                return Some(upper);
            }
        }
    }

    const NAMED: [&str; 18] = [
        "SPACE",
        "ENTER",
        "TAB",
        "BACKSPACE",
        "DELETE",
        "INSERT",
        "HOME",
        "END",
        "PAGEUP",
        "PAGEDOWN",
        "UP",
        "DOWN",
        "LEFT",
        "RIGHT",
        "COMMA",
        "PERIOD",
        "SLASH",
        "SEMICOLON",
    ];
    NAMED.contains(&upper.as_str()).then_some(upper)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_default_hotkey() {
        let hotkey = Hotkey::parse("Ctrl+Alt+R").expect("parses");
        assert_eq!(hotkey.modifiers, vec![Modifier::Ctrl, Modifier::Alt]);
        assert_eq!(hotkey.key, "R");
    }

    #[test]
    fn normalises_case_and_spacing() {
        let a = Hotkey::parse("ctrl + alt + r").expect("parses");
        let b = Hotkey::parse("CTRL+ALT+R").expect("parses");
        assert_eq!(a, b);
    }

    #[test]
    fn modifier_order_does_not_matter() {
        let a = Hotkey::parse("Alt+Ctrl+R").expect("parses");
        let b = Hotkey::parse("Ctrl+Alt+R").expect("parses");
        assert_eq!(a, b, "the same combination typed either way must match");
    }

    #[test]
    fn accepts_modifier_aliases() {
        assert_eq!(
            Hotkey::parse("Control+Option+R").expect("parses"),
            Hotkey::parse("Ctrl+Alt+R").expect("parses")
        );
    }

    #[test]
    fn a_bare_key_is_refused_because_it_would_fire_while_typing() {
        assert_eq!(Hotkey::parse("R"), Err(HotkeyError::NoModifier));
    }

    #[test]
    fn modifiers_alone_are_refused() {
        assert_eq!(Hotkey::parse("Ctrl+Alt"), Err(HotkeyError::NoKey));
    }

    #[test]
    fn empty_input_is_refused() {
        assert_eq!(Hotkey::parse(""), Err(HotkeyError::Empty));
        assert_eq!(Hotkey::parse("   "), Err(HotkeyError::Empty));
    }

    #[test]
    fn a_repeated_modifier_is_refused() {
        assert!(matches!(
            Hotkey::parse("Ctrl+Ctrl+R"),
            Err(HotkeyError::Duplicate(_))
        ));
    }

    #[test]
    fn two_non_modifier_keys_are_refused() {
        assert!(matches!(
            Hotkey::parse("Ctrl+R+Y"),
            Err(HotkeyError::Duplicate(_))
        ));
    }

    #[test]
    fn a_key_that_does_not_exist_is_refused_by_name() {
        assert_eq!(
            Hotkey::parse("Ctrl+Banana"),
            Err(HotkeyError::UnknownKey("Banana".to_owned()))
        );
    }

    #[test]
    fn function_keys_are_accepted_only_in_range() {
        assert_eq!(Hotkey::parse("Ctrl+F5").unwrap().key, "F5");
        assert_eq!(Hotkey::parse("Ctrl+F24").unwrap().key, "F24");
        assert!(Hotkey::parse("Ctrl+F25").is_err());
        assert!(Hotkey::parse("Ctrl+F0").is_err());
    }

    #[test]
    fn named_keys_are_accepted() {
        for key in ["Space", "Enter", "Home", "PageUp", "Left"] {
            let accelerator = format!("Ctrl+Alt+{key}");
            assert!(
                Hotkey::parse(&accelerator).is_ok(),
                "{accelerator} should parse"
            );
        }
    }

    #[test]
    fn a_hotkey_round_trips_through_its_accelerator() {
        let original = Hotkey::parse("Shift+Ctrl+F5").expect("parses");
        let reparsed = Hotkey::parse(&original.to_accelerator()).expect("reparses");
        assert_eq!(original, reparsed);
    }

    #[test]
    fn the_accelerator_uses_tokens_the_plugin_understands() {
        let hotkey = Hotkey::parse("ctrl+shift+r").expect("parses");
        assert_eq!(hotkey.to_accelerator(), "Ctrl+Shift+R");
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn a_hotkey_displays_readably_on_desktop_platforms() {
        let hotkey = Hotkey::parse("Ctrl+Alt+R").expect("parses");
        assert_eq!(hotkey.to_string(), "Ctrl + Alt + R");
    }

    #[test]
    fn portal_triggers_use_the_xdg_spelling() {
        assert_eq!(
            Hotkey::parse("Ctrl+Alt+R").unwrap().to_portal_trigger(),
            "CTRL+ALT+r"
        );
        assert_eq!(
            Hotkey::parse("Super+Shift+K").unwrap().to_portal_trigger(),
            "SHIFT+LOGO+k"
        );
        assert_eq!(
            Hotkey::parse("Ctrl+F5").unwrap().to_portal_trigger(),
            "CTRL+F5"
        );
    }

    #[test]
    fn every_error_explains_itself() {
        for error in [
            HotkeyError::Empty,
            HotkeyError::NoModifier,
            HotkeyError::NoKey,
            HotkeyError::UnknownKey("Q?".to_owned()),
            HotkeyError::Duplicate("Ctrl".to_owned()),
        ] {
            assert!(!error.to_string().is_empty());
        }
    }
}
