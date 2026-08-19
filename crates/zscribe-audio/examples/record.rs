use std::time::{Duration, Instant};
use zscribe_audio::{input_devices, read_mono, RecordOptions, Session};

fn main() {
    tracing_subscriber_init();

    let mut args = std::env::args().skip(1);
    let seconds: u64 = args.next().and_then(|a| a.parse().ok()).unwrap_or(3);
    let wanted = args.next();

    println!("Input devices:");
    let devices = input_devices();
    for device in &devices {
        println!(
            "  {} {}\n      {}",
            if device.is_default { "*" } else { " " },
            device.name,
            device.id
        );
    }

    let device_id = wanted.and_then(|wanted| {
        devices
            .iter()
            .find(|d| d.name.to_lowercase().contains(&wanted.to_lowercase()) || d.id == wanted)
            .map(|d| d.id.clone())
    });

    let output = std::env::temp_dir().join("zscribe-example.wav");
    println!("\nRecording {seconds}s to {}\n", output.display());

    let session = match Session::start(RecordOptions {
        device_id,
        system_source: std::env::var("ZSCRIBE_SYSTEM_SOURCE").ok(),
        exact_device: false,
        output: output.clone(),
        preroll: Vec::new(),
    }) {
        Ok(session) => session,
        Err(err) => {
            eprintln!("could not start: {err}\n  → {}", err.remedy());
            std::process::exit(1);
        }
    };

    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(seconds) {
        std::thread::sleep(Duration::from_millis(100));

        let level = session.level();
        let filled = (level.rms * 40.0).min(40.0) as usize;
        print!(
            "\r  {:>5.1}s [{:<40}] rms {:.3} peak {:.3}{}",
            session.duration_ms() as f32 / 1000.0,
            "#".repeat(filled),
            level.rms,
            level.peak,
            if level.is_clipping() {
                " CLIPPING"
            } else {
                "        "
            },
        );
        use std::io::Write;
        let _ = std::io::stdout().flush();

        if let Some(failure) = session.failure() {
            println!("\n\nthe stream failed: {failure}");
            break;
        }
    }

    let finished = session.stop().expect("stop");
    let samples = read_mono(std::path::Path::new(&finished.path)).expect("read back");
    let peak = samples.iter().fold(0.0f32, |max, s| max.max(s.abs()));

    println!("\n\nWrote {}", finished.path);
    println!("  duration  {} ms", finished.duration_ms);
    println!(
        "  samples   {} ({} expected at 16 kHz)",
        samples.len(),
        finished.duration_ms * 16
    );
    println!("  peak      {peak:.4}");

    if peak < 0.001 {
        println!("\n  Nothing was captured — the microphone is muted or the wrong one was used.");
    }
}

fn tracing_subscriber_init() {
    let _ = std::panic::catch_unwind(|| {});
}
