use std::str::FromStr;

use cpal::traits::{DeviceTrait, HostTrait};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::wav::SAMPLE_RATE;
use crate::AudioError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct InputDevice {
    pub id: String,

    pub name: String,

    pub is_default: bool,
}

fn name_of(device: &cpal::Device) -> Option<String> {
    device
        .description()
        .ok()
        .map(|description| description.name().to_owned())
}

fn id_of(device: &cpal::Device) -> Option<String> {
    device.id().ok().map(|id| id.to_string())
}

const PLUMBING_PRIORITY: u8 = 4;

fn priority(id: &str) -> u8 {
    if id.ends_with(":pulse") || id.ends_with(":pipewire") {
        return 0;
    }

    if id.contains(":hw:") || id.contains(":plughw:") || id.contains(":front:") {
        return 2;
    }

    const PLUMBING: [&str; 10] = [
        ":null",
        ":oss",
        ":jack",
        ":lavrate",
        ":samplerate",
        ":speexrate",
        ":speex",
        ":upmix",
        ":vdownmix",
        ":usbstream",
    ];
    if PLUMBING.iter().any(|suffix| id.contains(suffix)) {
        return 4;
    }

    1
}

pub fn input_devices() -> Vec<InputDevice> {
    const CACHE_FOR: std::time::Duration = std::time::Duration::from_secs(5);

    type Cached = std::sync::Mutex<Option<(std::time::Instant, Vec<InputDevice>)>>;
    static CACHE: std::sync::OnceLock<Cached> = std::sync::OnceLock::new();

    let cache = CACHE.get_or_init(|| std::sync::Mutex::new(None));
    {
        let held = cache.lock().expect("device cache poisoned");
        if let Some((measured, devices)) = held.as_ref() {
            if measured.elapsed() < CACHE_FOR {
                return devices.clone();
            }
        }
    }

    let listed = enumerate();
    *cache.lock().expect("device cache poisoned") =
        Some((std::time::Instant::now(), listed.clone()));
    listed
}

fn enumerate() -> Vec<InputDevice> {
    let host = cpal::default_host();
    let default_id = host.default_input_device().as_ref().and_then(id_of);

    let Ok(devices) = host.input_devices() else {
        tracing::warn!("the audio host would not enumerate input devices");
        return Vec::new();
    };

    let mut listed: Vec<InputDevice> = devices
        .filter_map(|device| {
            let id = id_of(&device)?;

            if priority(&id) >= PLUMBING_PRIORITY {
                return None;
            }

            let name = name_of(&device)?;
            let config = choose_config(&device).ok()?;

            if !can_open(&device, &config) {
                tracing::debug!(device = %id, "listed by the host but will not open");
                return None;
            }

            Some(InputDevice {
                is_default: Some(&id) == default_id.as_ref(),
                id,
                name,
            })
        })
        .collect();

    listed.sort_by(|a, b| {
        b.is_default
            .cmp(&a.is_default)
            .then(priority(&a.id).cmp(&priority(&b.id)))
            .then(a.name.cmp(&b.name))
    });

    let mut seen: Vec<String> = Vec::new();
    listed.retain(|device| {
        let fresh = !seen.contains(&device.name);
        if fresh {
            seen.push(device.name.clone());
        }
        fresh
    });

    if let Some(first) = listed.first_mut() {
        if !first.is_default {
            first.is_default = true;
        }
    }

    listed
}

fn can_open(device: &cpal::Device, config: &cpal::SupportedStreamConfig) -> bool {
    let stream_config: cpal::StreamConfig = (*config).into();

    match config.sample_format() {
        cpal::SampleFormat::F32 => device
            .build_input_stream(stream_config, |_: &[f32], _: &_| {}, |_| {}, None)
            .is_ok(),
        cpal::SampleFormat::I16 => device
            .build_input_stream(stream_config, |_: &[i16], _: &_| {}, |_| {}, None)
            .is_ok(),
        cpal::SampleFormat::U16 => device
            .build_input_stream(stream_config, |_: &[u16], _: &_| {}, |_| {}, None)
            .is_ok(),
        cpal::SampleFormat::I32 => device
            .build_input_stream(stream_config, |_: &[i32], _: &_| {}, |_| {}, None)
            .is_ok(),

        _ => true,
    }
}

pub struct Candidate {
    pub device: cpal::Device,
    pub config: cpal::SupportedStreamConfig,
    pub label: String,
}

pub fn candidates(preferred_id: Option<&str>, exact: bool) -> Result<Vec<Candidate>, AudioError> {
    let host = cpal::default_host();
    let mut out: Vec<Candidate> = Vec::new();
    let mut seen: Vec<String> = Vec::new();

    let mut push = |device: cpal::Device| {
        let Some(id) = id_of(&device) else { return };
        if seen.contains(&id) {
            return;
        }
        let Ok(config) = choose_config(&device) else {
            return;
        };
        seen.push(id.clone());
        out.push(Candidate {
            label: name_of(&device).unwrap_or(id),
            device,
            config,
        });
    };

    if let Some(wanted) = preferred_id {
        match cpal::DeviceId::from_str(wanted)
            .ok()
            .and_then(|id| host.device_by_id(&id))
        {
            Some(device) => push(device),
            None => tracing::warn!(device = wanted, "the chosen microphone is gone"),
        }
    }

    if exact {
        return match out.is_empty() {
            true => Err(AudioError::NoInputDevice),
            false => Ok(out),
        };
    }

    if let Some(device) = host.default_input_device() {
        push(device);
    }

    if let Ok(devices) = host.input_devices() {
        let mut rest: Vec<cpal::Device> = devices.collect();
        rest.sort_by_key(|device| id_of(device).map_or(u8::MAX, |id| priority(&id)));
        for device in rest {
            push(device);
        }
    }

    if out.is_empty() {
        return Err(AudioError::NoInputDevice);
    }
    Ok(out)
}

pub fn choose_config(device: &cpal::Device) -> Result<cpal::SupportedStreamConfig, AudioError> {
    let default = device
        .default_input_config()
        .map_err(|err| AudioError::Device(err.to_string()))?;

    let Ok(supported) = device.supported_input_configs() else {
        return Ok(default);
    };

    let native_16k = supported
        .filter(|range| range.sample_format() == default.sample_format())
        .filter(|range| {
            range.min_sample_rate() <= SAMPLE_RATE && range.max_sample_rate() >= SAMPLE_RATE
        })
        .min_by_key(|range| range.channels())
        .and_then(|range| range.try_with_sample_rate(SAMPLE_RATE));

    Ok(native_16k.unwrap_or(default))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hardware() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn listed() -> Vec<InputDevice> {
        let _hardware = hardware();
        input_devices()
    }

    #[test]
    fn enumerating_devices_never_panics_on_this_machine() {
        for device in listed() {
            assert!(!device.name.is_empty());
            assert!(!device.id.is_empty());
        }
    }

    #[test]
    fn exactly_one_device_is_marked_default_whenever_there_is_a_device() {
        let _hardware = hardware();
        let devices = input_devices();
        let count = devices.iter().filter(|d| d.is_default).count();

        if devices.is_empty() {
            assert_eq!(count, 0);
            return;
        }

        assert_eq!(count, 1, "the picker must name the device it will use");
    }

    #[test]
    fn the_default_device_is_listed_first() {
        let devices = listed();
        if devices.iter().any(|d| d.is_default) {
            assert!(
                devices[0].is_default,
                "the default belongs at the top of the picker"
            );
        }
    }

    #[test]
    fn the_same_sound_card_is_not_listed_once_per_alsa_alias() {
        let devices = listed();
        let mut names: Vec<&str> = devices.iter().map(|d| d.name.as_str()).collect();
        names.sort_unstable();
        let total = names.len();
        names.dedup();
        assert_eq!(names.len(), total, "the picker must not repeat a device");
    }

    #[test]
    fn device_ids_are_unique_so_settings_can_key_off_them() {
        let devices = listed();
        let mut ids: Vec<&str> = devices.iter().map(|d| d.id.as_str()).collect();
        ids.sort_unstable();
        let total = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), total);
    }

    #[test]
    fn every_listed_device_really_opens() {
        let _hardware = hardware();

        for listed in input_devices() {
            let candidates = candidates(Some(&listed.id), true)
                .unwrap_or_else(|err| panic!("{} is listed but: {err}", listed.name));

            let candidate = &candidates[0];
            assert!(
                can_open(&candidate.device, &candidate.config),
                "{} is in the picker but will not open",
                listed.name
            );
        }
    }

    #[test]
    fn plumbing_never_reaches_the_picker() {
        for device in listed() {
            assert!(
                priority(&device.id) < PLUMBING_PRIORITY,
                "{} is plumbing, not a microphone",
                device.id
            );
        }
    }

    #[test]
    fn repeated_enumeration_is_served_from_the_cache() {
        let first = listed();
        let started = std::time::Instant::now();
        let second = listed();

        assert_eq!(first, second);
        assert!(
            started.elapsed() < std::time::Duration::from_millis(20),
            "not cached"
        );
    }

    #[test]
    fn plumbing_is_ranked_below_real_microphones() {
        assert!(priority("alsa:pulse") < priority("alsa:default"));
        assert!(priority("alsa:pipewire") < priority("alsa:hw:CARD=1,DEV=0"));
        assert!(priority("alsa:hw:CARD=1,DEV=0") < priority("alsa:null"));
        assert!(priority("alsa:default") < priority("alsa:jack"));

        for plumbing in [
            "alsa:null",
            "alsa:samplerate",
            "alsa:upmix",
            "alsa:usbstream:CARD=HDMI",
        ] {
            assert_eq!(priority(plumbing), 4, "{plumbing}");
        }
    }

    #[test]
    fn an_unrecognised_platforms_ids_all_rank_the_same() {
        assert_eq!(
            priority("Microphone (USB Audio Device)"),
            priority("Built-in Microphone")
        );
    }

    #[test]
    fn there_is_always_more_than_one_candidate_to_fall_back_to() {
        let _hardware = hardware();
        let candidates = candidates(None, false).expect("this machine has audio devices");
        assert!(!candidates.is_empty());

        for candidate in &candidates {
            assert!(candidate.config.channels() >= 1);
            assert!(candidate.config.sample_rate() >= 8_000);
        }
    }

    #[test]
    fn the_chosen_device_is_tried_first() {
        let _hardware = hardware();
        let devices = input_devices();
        let Some(last) = devices.last() else { return };

        let candidates = candidates(Some(&last.id), false).expect("candidates");
        assert_eq!(
            id_of(&candidates[0].device).as_deref(),
            Some(last.id.as_str()),
            "the user's choice must be attempted before anything else"
        );
    }

    #[test]
    fn a_device_that_no_longer_exists_still_leaves_candidates_to_try() {
        let _hardware = hardware();
        let candidates =
            candidates(Some("alsa:a-device-that-was-never-plugged-in"), false).expect("candidates");
        assert!(
            !candidates.is_empty(),
            "an unplugged mic must not block recording"
        );
    }

    #[test]
    fn an_unparseable_device_id_is_ignored_rather_than_failing() {
        let _hardware = hardware();
        assert!(candidates(Some("not a device id at all"), false).is_ok());
    }

    #[test]
    fn no_candidate_is_offered_twice() {
        let _hardware = hardware();
        let devices = input_devices();
        let preferred = devices.first().map(|d| d.id.clone());

        let candidates = candidates(preferred.as_deref(), false).expect("candidates");
        let mut ids: Vec<String> = candidates.iter().filter_map(|c| id_of(&c.device)).collect();
        let total = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(
            ids.len(),
            total,
            "the preferred device must not repeat later in the list"
        );
    }

    #[test]
    fn a_named_source_refuses_to_record_somebody_elses_device() {
        let _hardware = hardware();

        assert!(matches!(
            candidates(Some("alsa:a-device-that-was-never-plugged-in"), true),
            Err(AudioError::NoInputDevice)
        ));
    }

    #[test]
    fn an_exact_request_for_a_device_that_exists_still_works() {
        let _hardware = hardware();
        let Some(device) = input_devices().into_iter().next() else {
            return;
        };
        let found = candidates(Some(&device.id), true).expect("the device is present");

        assert_eq!(found.len(), 1, "exact means this device and no other");
        assert_eq!(id_of(&found[0].device).as_deref(), Some(device.id.as_str()));
    }

    #[test]
    fn the_main_microphone_still_falls_back_when_it_disappears() {
        let _hardware = hardware();

        assert!(candidates(Some("alsa:gone"), false).is_ok());
    }

    #[test]
    fn the_chosen_format_is_never_worse_than_the_devices_own_default() {
        let _hardware = hardware();

        let host = cpal::default_host();
        let Ok(devices) = host.input_devices() else {
            return;
        };

        for device in devices {
            let (Ok(default), Ok(chosen)) = (device.default_input_config(), choose_config(&device))
            else {
                continue;
            };

            assert_eq!(
                chosen.sample_format(),
                default.sample_format(),
                "{:?} was downgraded from {:?} to {:?}",
                name_of(&device),
                default.sample_format(),
                chosen.sample_format(),
            );
        }
    }
}
