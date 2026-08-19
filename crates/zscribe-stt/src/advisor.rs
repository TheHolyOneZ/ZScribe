use serde::{Deserialize, Serialize};
use ts_rs::TS;
use zscribe_platform::{Acceleration, CapabilityNote, Machine};

use crate::models::{find, ModelSpec, MODELS, PREFERRED_MODEL_ID};

const VRAM_HEADROOM_MB: u64 = 512;

const RAM_HEADROOM_MB: u64 = 2_048;

const DISK_HEADROOM_MB: u64 = 1_024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Recommendation {
    pub model_id: String,

    pub accelerated: bool,

    pub headline: String,

    pub viable: Vec<String>,

    pub notes: Vec<CapabilityNote>,
}

pub fn recommend(machine: &Machine) -> Recommendation {
    let mut notes = Vec::new();

    let accelerated = machine.can_accelerate();
    let budget_mb = memory_budget(machine, accelerated);

    let viable: Vec<&'static ModelSpec> = MODELS
        .iter()
        .filter(|model| model.required_mb() <= budget_mb)
        .filter(|model| fits_on_disk(model, machine))
        .collect();

    let chosen = choose(&viable, accelerated);

    notes.extend(gpu_note(machine));
    notes.extend(disk_note(machine, chosen));
    notes.extend(memory_note(machine, budget_mb, chosen));
    notes.extend(speed_note(machine, chosen, accelerated));

    Recommendation {
        model_id: chosen.id.to_owned(),
        accelerated,
        headline: headline(machine, chosen, accelerated),
        viable: viable.iter().map(|model| model.id.to_owned()).collect(),
        notes,
    }
}

fn memory_budget(machine: &Machine, accelerated: bool) -> u64 {
    if accelerated {
        if let Some(gpu) = machine.best_gpu() {
            let vram = gpu.vram_mb.saturating_sub(VRAM_HEADROOM_MB);
            return if gpu.kind == zscribe_platform::GpuKind::Integrated {
                vram.min(machine.available_ram_mb.saturating_sub(RAM_HEADROOM_MB))
            } else {
                vram
            };
        }
    }
    machine.available_ram_mb.saturating_sub(RAM_HEADROOM_MB)
}

fn fits_on_disk(model: &ModelSpec, machine: &Machine) -> bool {
    machine.free_disk_mb == 0 || model.megabytes() + DISK_HEADROOM_MB <= machine.free_disk_mb
}

fn choose(viable: &[&'static ModelSpec], accelerated: bool) -> &'static ModelSpec {
    let smallest = find("tiny").expect("tiny is always in the registry");

    if viable.is_empty() {
        return smallest;
    }

    if accelerated {
        if let Some(preferred) = viable.iter().find(|m| m.id == PREFERRED_MODEL_ID) {
            return preferred;
        }

        return viable.last().copied().unwrap_or(smallest);
    }

    viable
        .iter()
        .rfind(|model| model.cpu_speed >= 3)
        .copied()
        .or_else(|| viable.first().copied())
        .unwrap_or(smallest)
}

fn headline(machine: &Machine, model: &ModelSpec, accelerated: bool) -> String {
    match (accelerated, machine.best_gpu()) {
        (true, Some(gpu)) => format!(
            "{} — {} MB. Your {} ({} GB) runs this through Vulkan at roughly {}× real time.",
            model.label,
            model.megabytes(),
            gpu.name,
            (gpu.vram_mb as f64 / 1024.0).round() as u64,
            model.gpu_speed,
        ),
        _ => format!(
            "{} — {} MB. No usable GPU was found, so transcription runs on your {} at roughly \
             {}× real time.",
            model.label,
            model.megabytes(),
            machine.cpu_model,
            model.cpu_speed,
        ),
    }
}

fn gpu_note(machine: &Machine) -> Option<CapabilityNote> {
    let backend = Machine::backend();

    match machine.acceleration {
        Acceleration::Available if machine.best_gpu().is_some() => None,

        Acceleration::Available => Some(
            CapabilityNote::info(
                "No hardware GPU found",
                format!(
                    "{backend} is working, but the only device it reports is a software \
                     renderer, which is slower than your processor. Transcription will use \
                     the CPU."
                ),
            )
            .with_remedy("Install your graphics card's driver to use it for transcription."),
        ),

        Acceleration::NoDriver => Some(
            CapabilityNote::info(
                format!("No {backend} driver"),
                format!(
                    "Transcription will run on the CPU. {backend} is how ZScribe uses a \
                     graphics card, and no {backend} driver is installed."
                ),
            )
            .with_remedy(driver_remedy()),
        ),

        Acceleration::NoDevices => Some(
            CapabilityNote::info(
                format!("{backend} found no graphics card"),
                format!(
                    "The {backend} driver is installed but reports no devices, so \
                     transcription will run on the CPU."
                ),
            )
            .with_remedy("Check that your graphics driver is installed and the card is enabled."),
        ),

        Acceleration::Failed => Some(
            CapabilityNote::degraded(
                format!("{backend} would not start"),
                format!(
                    "A {backend} driver is present but refused to initialise. Transcription \
                     will run on the CPU, which still works but is slower."
                ),
            )
            .with_remedy("Updating your graphics driver usually fixes this."),
        ),
    }
}

fn driver_remedy() -> &'static str {
    if cfg!(target_os = "windows") {
        "Install your graphics card's driver from AMD, NVIDIA or Intel — the Vulkan runtime \
         comes with it — then run this scan again."
    } else if cfg!(target_os = "macos") {
        "Metal is part of macOS, so this is unexpected. Updating macOS is the thing to try."
    } else {
        "Install your card's Vulkan driver — mesa-vulkan-drivers for AMD or Intel, the \
         proprietary driver for NVIDIA — then run this scan again."
    }
}

fn disk_note(machine: &Machine, model: &ModelSpec) -> Option<CapabilityNote> {
    if machine.free_disk_mb == 0 {
        return None;
    }

    let needed = model.megabytes() + DISK_HEADROOM_MB;
    if machine.free_disk_mb >= needed {
        return None;
    }

    Some(
        CapabilityNote::degraded(
            "Not enough disk space",
            format!(
                "{} needs {} MB plus room for your recordings, and only {} MB is free.",
                model.label,
                model.megabytes(),
                machine.free_disk_mb
            ),
        )
        .with_remedy("Free some space, or choose a smaller model below."),
    )
}

fn memory_note(machine: &Machine, budget_mb: u64, model: &ModelSpec) -> Option<CapabilityNote> {
    if budget_mb >= model.required_mb() {
        return None;
    }

    Some(
        CapabilityNote::degraded(
            "Not much memory to work with",
            format!(
                "{} wants about {} MB while it runs, and only {} MB looks available. It may be \
                 slow, or fail on a long recording.",
                model.label,
                model.required_mb(),
                budget_mb
            ),
        )
        .with_remedy(format!(
            "Close some applications, or pick a smaller model. Your machine has {} MB of memory \
             in total.",
            machine.total_ram_mb
        )),
    )
}

fn speed_note(machine: &Machine, model: &ModelSpec, accelerated: bool) -> Option<CapabilityNote> {
    let speed = if accelerated {
        model.gpu_speed
    } else {
        model.cpu_speed
    };

    if speed >= 2 {
        return None;
    }

    Some(
        CapabilityNote::degraded(
            "Transcription will be slow",
            format!(
                "On this machine {} runs at roughly {}× real time, so a one-hour recording takes \
                 about an hour to transcribe.",
                model.label,
                speed.max(1)
            ),
        )
        .with_remedy(if machine.can_accelerate() {
            "Choose a smaller model below."
        } else {
            "Choose a smaller model below, or install a Vulkan driver so your graphics card can \
             do the work."
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use zscribe_platform::{Gpu, GpuKind};

    fn gpu(name: &str, kind: GpuKind, vram_mb: u64) -> Gpu {
        Gpu {
            name: name.to_owned(),
            vendor: "AMD".to_owned(),
            kind,
            vram_mb,
        }
    }

    fn workstation() -> Machine {
        Machine {
            cpu_model: "AMD Ryzen 7 5700X 8-Core Processor".to_owned(),
            cpu_threads: 16,
            total_ram_mb: 32_007,
            available_ram_mb: 19_664,
            free_disk_mb: 30_805,
            acceleration: Acceleration::Available,
            gpus: vec![gpu(
                "AMD Radeon RX 6700 XT (RADV NAVI22)",
                GpuKind::Discrete,
                12_032,
            )],
        }
    }

    fn cpu_only(available_ram_mb: u64) -> Machine {
        Machine {
            cpu_model: "Intel Core i5-8250U".to_owned(),
            cpu_threads: 8,
            total_ram_mb: 8_192,
            available_ram_mb,
            free_disk_mb: 50_000,
            acceleration: Acceleration::NoDriver,
            gpus: Vec::new(),
        }
    }

    #[test]
    fn a_gaming_machine_is_recommended_turbo_on_the_gpu() {
        let recommendation = recommend(&workstation());

        assert_eq!(recommendation.model_id, "large-v3-turbo");
        assert!(recommendation.accelerated);
        assert!(
            recommendation.notes.is_empty(),
            "{:?}",
            recommendation.notes
        );
    }

    #[test]
    fn the_headline_names_the_actual_card_and_its_memory() {
        let headline = recommend(&workstation()).headline;

        assert!(headline.contains("Radeon RX 6700 XT"), "got: {headline}");

        assert!(headline.contains("12 GB"), "got: {headline}");
        assert!(headline.contains("Vulkan"), "got: {headline}");
        assert!(headline.contains("Large v3 Turbo"), "got: {headline}");
    }

    #[test]
    fn a_laptop_without_a_gpu_is_not_told_to_run_a_large_model() {
        let recommendation = recommend(&cpu_only(6_000));

        assert!(!recommendation.accelerated);
        assert_eq!(recommendation.model_id, "small");

        let headline = recommendation.headline;
        assert!(
            headline.contains("CPU") || headline.contains("i5-8250U"),
            "got: {headline}"
        );
    }

    #[test]
    fn a_machine_with_no_vulkan_is_told_how_to_get_it() {
        let recommendation = recommend(&cpu_only(6_000));

        let note = recommendation
            .notes
            .iter()
            .find(|n| n.title.contains("Vulkan"))
            .expect("must mention the missing driver");

        assert!(
            note.remedy.as_ref().is_some_and(|r| r.contains("driver")),
            "{note:?}"
        );
    }

    #[test]
    fn a_software_rasteriser_is_not_treated_as_a_graphics_card() {
        let machine = Machine {
            gpus: vec![gpu("llvmpipe", GpuKind::Software, 8_000)],
            acceleration: Acceleration::Available,
            ..cpu_only(16_000)
        };
        let recommendation = recommend(&machine);

        assert!(!recommendation.accelerated);
        assert!(recommendation
            .notes
            .iter()
            .any(|n| n.detail.contains("software renderer")));
    }

    #[test]
    fn a_tiny_machine_still_gets_a_working_recommendation() {
        let machine = Machine {
            total_ram_mb: 2_048,
            available_ram_mb: 900,
            free_disk_mb: 400,
            ..cpu_only(900)
        };
        let recommendation = recommend(&machine);

        assert_eq!(recommendation.model_id, "tiny");
        assert!(!recommendation.model_id.is_empty());
    }

    #[test]
    fn a_full_disk_is_reported_with_the_numbers() {
        let machine = Machine {
            free_disk_mb: 200,
            ..workstation()
        };
        let recommendation = recommend(&machine);

        let note = recommendation
            .notes
            .iter()
            .find(|n| n.title.contains("disk"))
            .expect("must warn about the disk");
        assert!(
            note.detail.contains("200"),
            "the user needs the real figure: {note:?}"
        );
    }

    #[test]
    fn a_disk_probe_that_failed_does_not_invent_a_warning() {
        let machine = Machine {
            free_disk_mb: 0,
            ..workstation()
        };
        let recommendation = recommend(&machine);

        assert!(!recommendation
            .notes
            .iter()
            .any(|n| n.title.contains("disk")));
        assert_eq!(recommendation.model_id, "large-v3-turbo");
    }

    #[test]
    fn an_integrated_gpu_cannot_claim_more_memory_than_the_system_has() {
        let machine = Machine {
            total_ram_mb: 8_192,
            available_ram_mb: 3_000,
            acceleration: Acceleration::Available,
            gpus: vec![gpu("Radeon Graphics", GpuKind::Integrated, 16_000)],
            ..cpu_only(3_000)
        };
        let recommendation = recommend(&machine);

        assert!(recommendation.accelerated);
        assert!(
            !recommendation.viable.contains(&"large-v3".to_owned()),
            "3 GB of weights do not fit in 3 GB of shared memory"
        );
    }

    #[test]
    fn a_discrete_card_is_preferred_over_an_integrated_one() {
        let machine = Machine {
            gpus: vec![
                gpu("Radeon Graphics", GpuKind::Integrated, 2_048),
                gpu("Radeon RX 6700 XT", GpuKind::Discrete, 12_032),
            ],
            ..workstation()
        };
        assert!(recommend(&machine).headline.contains("RX 6700 XT"));
    }

    #[test]
    fn a_vulkan_driver_that_will_not_start_is_flagged_as_degraded() {
        let machine = Machine {
            acceleration: Acceleration::Failed,
            gpus: Vec::new(),
            ..workstation()
        };
        let recommendation = recommend(&machine);

        assert!(!recommendation.accelerated);
        assert!(recommendation
            .notes
            .iter()
            .any(|n| n.severity == zscribe_platform::NoteSeverity::Degraded));
    }

    #[test]
    fn every_note_carries_a_remedy() {
        for machine in [
            workstation(),
            cpu_only(900),
            cpu_only(6_000),
            Machine {
                free_disk_mb: 100,
                ..workstation()
            },
            Machine {
                acceleration: Acceleration::Failed,
                gpus: Vec::new(),
                ..workstation()
            },
            Machine {
                acceleration: Acceleration::NoDevices,
                gpus: Vec::new(),
                ..workstation()
            },
        ] {
            for note in recommend(&machine).notes {
                assert!(!note.title.is_empty());
                assert!(!note.detail.is_empty());
                assert!(note.remedy.is_some(), "no remedy for: {}", note.title);
            }
        }
    }

    #[test]
    fn the_recommended_model_is_always_one_that_exists() {
        for machine in [workstation(), cpu_only(900), cpu_only(64_000)] {
            let recommendation = recommend(&machine);
            assert!(
                find(&recommendation.model_id).is_some(),
                "recommended a model that is not in the registry: {}",
                recommendation.model_id
            );
        }
    }

    #[test]
    fn the_recommended_model_is_among_the_viable_ones_when_any_are() {
        let recommendation = recommend(&workstation());
        assert!(recommendation.viable.contains(&recommendation.model_id));
    }

    #[test]
    fn a_slow_pairing_warns_with_a_figure_the_user_can_picture() {
        let machine = Machine {
            available_ram_mb: 6_000,
            acceleration: Acceleration::NoDriver,
            gpus: Vec::new(),
            ..workstation()
        };

        let slow = Machine {
            available_ram_mb: 3_000,
            ..machine
        };
        let recommendation = recommend(&slow);

        if let Some(note) = recommendation
            .notes
            .iter()
            .find(|n| n.title.contains("slow"))
        {
            assert!(note.detail.contains("one-hour"), "{note:?}");
        }
    }
}
