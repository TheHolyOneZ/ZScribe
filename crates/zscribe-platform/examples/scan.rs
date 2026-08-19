use zscribe_platform::{Capabilities, Machine};

fn main() {
    let machine = Machine::probe(std::path::Path::new("."));

    println!(
        "CPU        {} ({} threads)",
        machine.cpu_model, machine.cpu_threads
    );
    println!(
        "RAM        {} MB total, {} MB available",
        machine.total_ram_mb, machine.available_ram_mb
    );
    println!("Disk       {} MB free here", machine.free_disk_mb);
    println!("{:<10} {:?}", Machine::backend(), machine.acceleration);

    if machine.gpus.is_empty() {
        println!("GPU        none enumerated");
    }
    for gpu in &machine.gpus {
        println!(
            "GPU        {} — {}, {}, {} MB",
            gpu.name,
            gpu.vendor,
            gpu.kind.label(),
            gpu.vram_mb
        );
    }

    match machine.best_gpu() {
        Some(gpu) => println!("Chosen     {} ({} MB)", gpu.name, gpu.vram_mb),
        None => println!("Chosen     CPU only"),
    }
    println!("Threads    {} for whisper", machine.whisper_threads());

    let caps = Capabilities::detect();
    println!(
        "\nSession    {:?}, hotkey via {:?}",
        caps.display_server, caps.hotkey
    );
    for note in &caps.notes {
        println!("  [{:?}] {}", note.severity, note.title);
        println!("    {}", note.detail);
        if let Some(remedy) = &note.remedy {
            println!("    → {remedy}");
        }
    }
}
