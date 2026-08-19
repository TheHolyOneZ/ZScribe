use crate::AudioError;

#[cfg(target_os = "linux")]
use std::sync::{Mutex, MutexGuard, OnceLock};

#[cfg(target_os = "linux")]
pub const PULSE_DEVICE: &str = "alsa:pulse";

#[cfg(target_os = "linux")]
const PULSE_SOURCE: &str = "PULSE_SOURCE";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemAudioSource {
    pub id: String,

    pub name: String,
}

#[cfg(target_os = "linux")]
pub fn system_audio_sources() -> Vec<SystemAudioSource> {
    let Ok(output) = std::process::Command::new("pactl")
        .args(["list", "sources"])
        .output()
    else {
        tracing::debug!("pactl is not available; system audio cannot be offered");
        return Vec::new();
    };

    if !output.status.success() {
        return Vec::new();
    }

    parse_sources(&String::from_utf8_lossy(&output.stdout))
}

#[cfg(not(target_os = "linux"))]
pub fn system_audio_sources() -> Vec<SystemAudioSource> {
    use cpal::traits::{DeviceTrait, HostTrait};

    let host = cpal::default_host();
    let Ok(devices) = host.output_devices() else {
        tracing::debug!("the audio host would not enumerate output devices");
        return Vec::new();
    };

    devices
        .filter_map(|device| {
            let id = device.id().ok()?.to_string();
            let name = device
                .description()
                .ok()
                .map(|description| description.name().to_owned())
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| friendly_name(&id));

            device.default_output_config().ok()?;

            Some(SystemAudioSource { id, name })
        })
        .collect()
}

#[cfg(target_os = "linux")]
pub fn candidates(_source: &str) -> Result<Vec<crate::device::Candidate>, AudioError> {
    crate::device::candidates(Some(PULSE_DEVICE), true)
}

#[cfg(not(target_os = "linux"))]
pub fn candidates(source: &str) -> Result<Vec<crate::device::Candidate>, AudioError> {
    use cpal::traits::{DeviceTrait, HostTrait};
    use std::str::FromStr;

    let host = cpal::default_host();
    let device = cpal::DeviceId::from_str(source)
        .ok()
        .and_then(|id| host.device_by_id(&id))
        .ok_or(AudioError::NoInputDevice)?;

    let config = device
        .default_output_config()
        .map_err(|err| AudioError::Device(err.to_string()))?;

    let label = device
        .description()
        .ok()
        .map(|description| description.name().to_owned())
        .unwrap_or_else(|| source.to_owned());

    Ok(vec![crate::device::Candidate {
        device,
        config,
        label,
    }])
}

#[cfg(target_os = "linux")]
fn parse_sources(listing: &str) -> Vec<SystemAudioSource> {
    let mut out = Vec::new();
    let mut id: Option<String> = None;
    let mut description: Option<String> = None;

    let finish = |id: &mut Option<String>,
                  description: &mut Option<String>,
                  out: &mut Vec<SystemAudioSource>| {
        let Some(name) = id.take() else {
            description.take();
            return;
        };

        if !name.ends_with(".monitor") {
            description.take();
            return;
        }

        let label = description
            .take()
            .map(|text| clean_description(&text))
            .filter(|text| !text.is_empty())
            .unwrap_or_else(|| friendly_name(&name));

        out.push(SystemAudioSource {
            id: name,
            name: label,
        });
    };

    for line in listing.lines() {
        let trimmed = line.trim();

        if let Some(value) = trimmed.strip_prefix("Name: ") {
            finish(&mut id, &mut description, &mut out);
            id = Some(value.trim().to_owned());
        } else if let Some(value) = trimmed.strip_prefix("Description: ") {
            description = Some(value.trim().to_owned());
        }
    }
    finish(&mut id, &mut description, &mut out);

    out
}

#[cfg(target_os = "linux")]
fn clean_description(description: &str) -> String {
    description
        .strip_prefix("Monitor of ")
        .unwrap_or(description)
        .trim()
        .to_owned()
}

fn friendly_name(id: &str) -> String {
    let stem = id.strip_suffix(".monitor").unwrap_or(id);

    let described = stem
        .strip_prefix("alsa_output.")
        .or_else(|| stem.strip_prefix("alsa_input."))
        .unwrap_or(stem);

    let mut words: Vec<String> = Vec::new();
    for part in described.split(['.', '-', '_']) {
        if part.is_empty()
            || part.eq_ignore_ascii_case("pci")
            || part.eq_ignore_ascii_case("usb")
            || part.eq_ignore_ascii_case("platform")
            || part.contains(':')
            || part.chars().all(|c| c.is_ascii_hexdigit())
        {
            continue;
        }

        let word = match part.to_ascii_lowercase().as_str() {
            "hdmi" => "HDMI".to_owned(),
            "analog" => "Analog".to_owned(),
            "stereo" => "Stereo".to_owned(),
            "pro" => "Pro".to_owned(),
            other => {
                let mut chars = other.chars();
                match chars.next() {
                    Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                    None => continue,
                }
            }
        };

        if !words.contains(&word) {
            words.push(word);
        }
    }

    if words.is_empty() {
        return "System audio".to_owned();
    }
    words.join(" ")
}

#[cfg(target_os = "linux")]
fn source_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

#[cfg(target_os = "linux")]
pub fn with_source(source: &str) -> SourceGuard {
    let guard = source_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let previous = std::env::var(PULSE_SOURCE).ok();

    std::env::set_var(PULSE_SOURCE, source);

    SourceGuard {
        _guard: guard,
        previous,
    }
}

#[cfg(not(target_os = "linux"))]
pub fn with_source(_source: &str) -> SourceGuard {
    SourceGuard
}

#[cfg(target_os = "linux")]
pub struct SourceGuard {
    _guard: MutexGuard<'static, ()>,
    previous: Option<String>,
}

#[cfg(not(target_os = "linux"))]
pub struct SourceGuard;

#[cfg(target_os = "linux")]
impl Drop for SourceGuard {
    fn drop(&mut self) {
        match &self.previous {
            Some(value) => std::env::set_var(PULSE_SOURCE, value),
            None => std::env::remove_var(PULSE_SOURCE),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monitor_names_become_something_a_person_would_recognise() {
        assert_eq!(
            friendly_name("alsa_output.pci-0000_09_00.1.hdmi-stereo-extra3.monitor"),
            "HDMI Stereo Extra3"
        );
    }

    #[test]
    fn an_analog_output_reads_plainly() {
        assert_eq!(
            friendly_name("alsa_output.pci-0000_00_1f.3.analog-stereo.monitor"),
            "Analog Stereo"
        );
    }

    #[test]
    fn a_name_that_reduces_to_nothing_still_says_something() {
        assert_eq!(
            friendly_name("alsa_output.pci-0000_00_1f.3.monitor"),
            "System audio"
        );
    }

    #[test]
    fn a_usb_device_keeps_its_model_name() {
        let name = friendly_name("alsa_output.usb-Focusrite_Scarlett-00.analog-stereo.monitor");
        assert!(
            name.contains("Focusrite") || name.contains("Scarlett"),
            "got: {name}"
        );
    }

    #[test]
    fn the_sound_servers_own_name_is_preferred_over_one_derived_from_the_id() {
        let listing = "\
Source #1078
\tState: RUNNING
\tName: alsa_output.usb-GuangZhou_FiiO_Electronics_Co._Ltd_FiiO_K5_Pro-00.analog-stereo.monitor
\tDescription: Monitor of FiiO K5 Pro Analog Stereo
";
        let sources = parse_sources(listing);

        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].name, "FiiO K5 Pro Analog Stereo");
    }

    #[test]
    fn microphones_are_left_out_of_the_system_audio_list() {
        let listing = "\
Source #1
\tName: alsa_input.usb-Some_Microphone-00.analog-stereo
\tDescription: Some Microphone
Source #2
\tName: alsa_output.hdmi.monitor
\tDescription: Monitor of HDMI
";
        let sources = parse_sources(listing);

        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].id, "alsa_output.hdmi.monitor");
    }

    #[test]
    fn a_source_without_a_description_falls_back_to_the_derived_name() {
        let listing = "\tName: alsa_output.pci-0000_09_00.1.hdmi-stereo-extra3.monitor\n";
        let sources = parse_sources(listing);

        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].name, "HDMI Stereo Extra3");
    }

    #[test]
    fn several_outputs_are_all_listed() {
        let listing = "\
\tName: alsa_output.hdmi.monitor
\tDescription: Monitor of HDMI
\tName: alsa_output.usb-FiiO.monitor
\tDescription: Monitor of FiiO K5 Pro
";
        let sources = parse_sources(listing);

        assert_eq!(sources.len(), 2);
        assert_eq!(sources[1].name, "FiiO K5 Pro");
    }

    #[test]
    fn an_empty_listing_yields_nothing_rather_than_panicking() {
        assert!(parse_sources("").is_empty());
    }

    #[test]
    fn enumerating_system_audio_never_panics_on_this_machine() {
        for source in system_audio_sources() {
            assert!(source.id.ends_with(".monitor"), "{}", source.id);
            assert!(!source.name.is_empty());
        }
    }

    #[test]
    fn only_monitors_are_offered_not_microphones() {
        assert!(system_audio_sources()
            .iter()
            .all(|s| s.id.ends_with(".monitor")));
    }

    #[test]
    fn the_source_variable_is_restored_afterwards() {
        std::env::remove_var(PULSE_SOURCE);
        {
            let _guard = with_source("something.monitor");
            assert_eq!(
                std::env::var(PULSE_SOURCE).as_deref(),
                Ok("something.monitor")
            );
        }
        assert!(
            std::env::var(PULSE_SOURCE).is_err(),
            "must not leak into later opens"
        );
    }

    #[test]
    fn an_existing_source_variable_is_put_back() {
        std::env::set_var(PULSE_SOURCE, "original");
        {
            let _guard = with_source("temporary.monitor");
            assert_eq!(
                std::env::var(PULSE_SOURCE).as_deref(),
                Ok("temporary.monitor")
            );
        }
        assert_eq!(std::env::var(PULSE_SOURCE).as_deref(), Ok("original"));
        std::env::remove_var(PULSE_SOURCE);
    }
}
