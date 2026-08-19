use serde::{Deserialize, Serialize};
use std::path::Path;
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum GpuKind {
    Discrete,

    Integrated,

    Virtual,

    Software,

    Other,
}

impl GpuKind {
    pub const fn is_worth_using(self) -> bool {
        matches!(self, GpuKind::Discrete | GpuKind::Integrated)
    }

    pub const fn label(self) -> &'static str {
        match self {
            GpuKind::Discrete => "discrete",
            GpuKind::Integrated => "integrated",
            GpuKind::Virtual => "virtual",
            GpuKind::Software => "software",
            GpuKind::Other => "unknown",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Gpu {
    pub name: String,

    pub vendor: String,

    pub kind: GpuKind,

    #[ts(type = "number")]
    pub vram_mb: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Acceleration {
    Available,

    NoDriver,

    NoDevices,

    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Machine {
    pub cpu_model: String,

    pub cpu_threads: usize,

    #[ts(type = "number")]
    pub total_ram_mb: u64,

    #[ts(type = "number")]
    pub available_ram_mb: u64,

    #[ts(type = "number")]
    pub free_disk_mb: u64,

    pub acceleration: Acceleration,
    pub gpus: Vec<Gpu>,
}

impl Machine {
    pub fn probe(models_dir: &Path) -> Self {
        let (cpu_model, total_ram_mb, available_ram_mb) = system_info();
        let (acceleration, gpus) = accelerator(&cpu_model, total_ram_mb);

        Self {
            cpu_model,
            cpu_threads: std::thread::available_parallelism().map_or(1, |n| n.get()),
            total_ram_mb,
            available_ram_mb,
            free_disk_mb: free_disk_mb(models_dir),
            acceleration,
            gpus,
        }
    }

    pub const fn backend() -> &'static str {
        if cfg!(target_os = "macos") {
            "Metal"
        } else {
            "Vulkan"
        }
    }

    pub fn best_gpu(&self) -> Option<&Gpu> {
        self.gpus
            .iter()
            .filter(|gpu| gpu.kind.is_worth_using())
            .max_by_key(|gpu| (gpu.kind == GpuKind::Discrete, gpu.vram_mb))
    }

    pub fn can_accelerate(&self) -> bool {
        self.acceleration == Acceleration::Available && self.best_gpu().is_some()
    }

    pub fn whisper_threads(&self) -> usize {
        self.cpu_threads.saturating_sub(1).clamp(1, 8)
    }
}

fn system_info() -> (String, u64, u64) {
    use sysinfo::{CpuRefreshKind, MemoryRefreshKind, RefreshKind, System};

    let system = System::new_with_specifics(
        RefreshKind::nothing()
            .with_memory(MemoryRefreshKind::nothing().with_ram())
            .with_cpu(CpuRefreshKind::nothing()),
    );

    let cpu_model = system
        .cpus()
        .first()
        .map(|cpu| cpu.brand().trim().to_owned())
        .filter(|brand| !brand.is_empty())
        .unwrap_or_else(|| "Unknown CPU".to_owned());

    (
        cpu_model,
        to_mb(system.total_memory()),
        to_mb(system.available_memory()),
    )
}

fn free_disk_mb(path: &Path) -> u64 {
    use sysinfo::Disks;

    let Some(path) = absolute_existing(path) else {
        return 0;
    };
    let disks = Disks::new_with_refreshed_list();

    disks
        .list()
        .iter()
        .filter(|disk| path.starts_with(disk.mount_point()))
        .max_by_key(|disk| disk.mount_point().as_os_str().len())
        .map(|disk| to_mb(disk.available_space()))
        .unwrap_or(0)
}

fn absolute_existing(path: &Path) -> Option<std::path::PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().ok()?.join(path)
    };

    let mut candidate = absolute.as_path();
    loop {
        if let Ok(resolved) = candidate.canonicalize() {
            return Some(resolved);
        }
        candidate = candidate.parent()?;
    }
}

fn to_mb(bytes: u64) -> u64 {
    bytes / (1024 * 1024)
}

#[cfg(not(target_os = "macos"))]
fn accelerator(_cpu_model: &str, _total_ram_mb: u64) -> (Acceleration, Vec<Gpu>) {
    vulkan::probe()
}

#[cfg(target_os = "macos")]
fn accelerator(cpu_model: &str, total_ram_mb: u64) -> (Acceleration, Vec<Gpu>) {
    let apple_silicon = cfg!(target_arch = "aarch64");

    let name = if apple_silicon && !cpu_model.is_empty() {
        cpu_model.to_owned()
    } else {
        "Metal GPU".to_owned()
    };

    (
        Acceleration::Available,
        vec![Gpu {
            name,
            vendor: "Apple".to_owned(),

            kind: GpuKind::Integrated,
            vram_mb: total_ram_mb,
        }],
    )
}

#[cfg(not(target_os = "macos"))]
mod vulkan {
    use super::{Acceleration, Gpu, GpuKind};
    use ash::vk;

    pub fn probe() -> (Acceleration, Vec<Gpu>) {
        let entry = match unsafe { ash::Entry::load() } {
            Ok(entry) => entry,
            Err(err) => {
                tracing::info!(%err, "no Vulkan loader; transcription will use the CPU");
                return (Acceleration::NoDriver, Vec::new());
            }
        };

        let app_info = vk::ApplicationInfo::default().api_version(vk::make_api_version(0, 1, 0, 0));
        let create_info = vk::InstanceCreateInfo::default().application_info(&app_info);

        let instance = match unsafe { entry.create_instance(&create_info, None) } {
            Ok(instance) => instance,
            Err(err) => {
                tracing::warn!(%err, "Vulkan loader present but the instance would not start");
                return (Acceleration::Failed, Vec::new());
            }
        };

        let gpus = unsafe {
            let devices = instance.enumerate_physical_devices().unwrap_or_default();
            let gpus: Vec<Gpu> = devices
                .into_iter()
                .map(|device| describe(&instance, device))
                .collect();
            instance.destroy_instance(None);
            gpus
        };

        if gpus.is_empty() {
            return (Acceleration::NoDevices, gpus);
        }
        (Acceleration::Available, gpus)
    }

    unsafe fn describe(instance: &ash::Instance, device: vk::PhysicalDevice) -> Gpu {
        let properties = unsafe { instance.get_physical_device_properties(device) };
        let memory = unsafe { instance.get_physical_device_memory_properties(device) };

        let vram_mb = memory.memory_heaps[..memory.memory_heap_count as usize]
            .iter()
            .filter(|heap| heap.flags.contains(vk::MemoryHeapFlags::DEVICE_LOCAL))
            .map(|heap| heap.size / (1024 * 1024))
            .max()
            .unwrap_or(0);

        Gpu {
            name: device_name(&properties.device_name),
            vendor: vendor_name(properties.vendor_id),
            kind: match properties.device_type {
                vk::PhysicalDeviceType::DISCRETE_GPU => GpuKind::Discrete,
                vk::PhysicalDeviceType::INTEGRATED_GPU => GpuKind::Integrated,
                vk::PhysicalDeviceType::VIRTUAL_GPU => GpuKind::Virtual,
                vk::PhysicalDeviceType::CPU => GpuKind::Software,
                _ => GpuKind::Other,
            },
            vram_mb,
        }
    }

    fn device_name(raw: &[std::ffi::c_char]) -> String {
        let bytes: Vec<u8> = raw
            .iter()
            .take_while(|byte| **byte != 0)
            .map(|byte| *byte as u8)
            .collect();

        let name = String::from_utf8_lossy(&bytes).trim().to_owned();
        if name.is_empty() {
            "Unknown device".to_owned()
        } else {
            name
        }
    }

    fn vendor_name(vendor_id: u32) -> String {
        match vendor_id {
            0x1002 | 0x1022 => "AMD".to_owned(),
            0x10DE => "NVIDIA".to_owned(),
            0x8086 => "Intel".to_owned(),
            0x13B5 => "ARM".to_owned(),
            0x5143 => "Qualcomm".to_owned(),
            0x106B => "Apple".to_owned(),
            other => format!("Vendor 0x{other:04X}"),
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn known_vendors_are_named_and_unknown_ones_keep_their_id() {
            assert_eq!(vendor_name(0x1002), "AMD");
            assert_eq!(vendor_name(0x10DE), "NVIDIA");
            assert_eq!(vendor_name(0x8086), "Intel");
            assert_eq!(vendor_name(0xBEEF), "Vendor 0xBEEF");
        }

        #[test]
        fn a_device_name_stops_at_the_nul_terminator() {
            let mut raw = [0 as std::ffi::c_char; 32];
            for (slot, byte) in raw.iter_mut().zip(b"Radeon RX 6700 XT") {
                *slot = *byte as std::ffi::c_char;
            }
            assert_eq!(device_name(&raw), "Radeon RX 6700 XT");
        }

        #[test]
        fn an_empty_device_name_does_not_render_as_a_blank_line() {
            assert_eq!(device_name(&[0 as std::ffi::c_char; 8]), "Unknown device");
        }

        #[test]
        fn probing_the_real_system_never_panics_whatever_is_installed() {
            let (status, gpus) = probe();
            assert_eq!(status == Acceleration::Available, !gpus.is_empty());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gpu(name: &str, kind: GpuKind, vram_mb: u64) -> Gpu {
        Gpu {
            name: name.to_owned(),
            vendor: "AMD".to_owned(),
            kind,
            vram_mb,
        }
    }

    fn machine(gpus: Vec<Gpu>) -> Machine {
        Machine {
            cpu_model: "Test CPU".to_owned(),
            cpu_threads: 12,
            total_ram_mb: 32_000,
            available_ram_mb: 20_000,
            free_disk_mb: 100_000,
            acceleration: if gpus.is_empty() {
                Acceleration::NoDevices
            } else {
                Acceleration::Available
            },
            gpus,
        }
    }

    #[test]
    fn a_discrete_card_is_chosen_over_an_integrated_one() {
        let m = machine(vec![
            gpu("Radeon Graphics", GpuKind::Integrated, 16_000),
            gpu("Radeon RX 6700 XT", GpuKind::Discrete, 12_288),
        ]);
        assert_eq!(m.best_gpu().expect("a gpu").name, "Radeon RX 6700 XT");
        assert!(m.can_accelerate());
    }

    #[test]
    fn the_larger_card_wins_between_two_of_the_same_kind() {
        let m = machine(vec![
            gpu("Small", GpuKind::Discrete, 4_096),
            gpu("Large", GpuKind::Discrete, 12_288),
        ]);
        assert_eq!(m.best_gpu().expect("a gpu").name, "Large");
    }

    #[test]
    fn a_software_rasteriser_is_never_recommended() {
        let m = machine(vec![gpu("llvmpipe", GpuKind::Software, 8_000)]);
        assert!(m.best_gpu().is_none());
        assert!(!m.can_accelerate());
    }

    #[test]
    fn an_integrated_gpu_alone_is_still_worth_using() {
        let m = machine(vec![gpu("Iris Xe", GpuKind::Integrated, 2_048)]);
        assert!(m.can_accelerate());
    }

    #[test]
    fn a_machine_with_no_vulkan_cannot_accelerate() {
        let m = Machine {
            acceleration: Acceleration::NoDriver,
            ..machine(Vec::new())
        };
        assert!(!m.can_accelerate());
        assert!(m.best_gpu().is_none());
    }

    #[test]
    fn a_gpu_list_that_survives_a_failed_loader_still_does_not_accelerate() {
        let m = Machine {
            acceleration: Acceleration::Failed,
            ..machine(vec![gpu("Radeon RX 6700 XT", GpuKind::Discrete, 12_288)])
        };
        assert!(!m.can_accelerate());
    }

    #[test]
    fn whisper_leaves_a_core_free_and_never_asks_for_more_than_eight() {
        assert_eq!(machine(Vec::new()).whisper_threads(), 8, "12 threads");

        let quad = Machine {
            cpu_threads: 4,
            ..machine(Vec::new())
        };
        assert_eq!(quad.whisper_threads(), 3);
    }

    #[test]
    fn a_single_core_machine_still_gets_one_thread() {
        let tiny = Machine {
            cpu_threads: 1,
            ..machine(Vec::new())
        };
        assert_eq!(tiny.whisper_threads(), 1);
    }

    #[test]
    fn probing_this_machine_reports_something_plausible() {
        let m = Machine::probe(Path::new("."));

        assert!(m.cpu_threads >= 1);
        assert!(!m.cpu_model.is_empty());
        assert!(m.total_ram_mb > 0, "a machine running tests has memory");
        assert!(m.available_ram_mb <= m.total_ram_mb);
    }

    #[test]
    fn free_space_is_measured_for_a_relative_path_too() {
        assert!(
            free_disk_mb(Path::new(".")) > 0,
            "the disk running these tests is not full"
        );
    }

    #[test]
    fn free_space_falls_back_to_the_nearest_existing_ancestor() {
        let temp = std::env::temp_dir();
        let missing = temp.join("zscribe-does-not-exist").join("models");

        assert_eq!(free_disk_mb(&missing), free_disk_mb(&temp));
    }

    #[test]
    fn a_path_on_no_filesystem_reports_zero_rather_than_panicking() {
        assert_eq!(
            free_disk_mb(Path::new("/nonexistent-root-xyz")),
            free_disk_mb(Path::new("/"))
        );
    }
}
