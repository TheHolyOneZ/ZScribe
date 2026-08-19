use std::time::{Duration, Instant};
use zscribe_audio::{input_devices, read_mono, RecordOptions, Session};

struct Track {
    label: String,
    session: Session,
}

fn main() {
    let mut args = std::env::args().skip(1);
    let seconds: u64 = args.next().and_then(|a| a.parse().ok()).unwrap_or(5);
    let wanted: Vec<String> = args.collect();

    let devices = input_devices();
    println!("Input devices:");
    for device in &devices {
        println!("   {}\n      {}", device.name, device.id);
    }

    let sources: Vec<(String, Option<String>, Option<String>)> = if wanted.is_empty() {
        devices
            .iter()
            .map(|d| (d.name.clone(), Some(d.id.clone()), None))
            .collect()
    } else {
        wanted
            .iter()
            .map(|arg| match arg.strip_prefix("system:") {
                Some(source) => (source.to_owned(), None, Some(source.to_owned())),
                None => {
                    let found = devices.iter().find(|d| {
                        d.id == *arg || d.name.to_lowercase().contains(&arg.to_lowercase())
                    });
                    match found {
                        Some(device) => (device.name.clone(), Some(device.id.clone()), None),

                        None => (arg.clone(), Some(arg.clone()), None),
                    }
                }
            })
            .collect()
    };

    let dir = std::env::temp_dir().join("zscribe-tracks");
    let _ = std::fs::create_dir_all(&dir);

    println!("\nOpening {} source(s) for {seconds}s\n", sources.len());

    let mut tracks: Vec<Track> = Vec::new();
    for (index, (label, device_id, system_source)) in sources.into_iter().enumerate() {
        let output = dir.join(format!("track-{index}.wav"));

        match Session::start(RecordOptions {
            device_id,
            system_source,
            exact_device: true,
            output: output.clone(),
            preroll: Vec::new(),
        }) {
            Ok(session) => {
                println!("  opened   {label}\n             → {}", output.display());
                tracks.push(Track { label, session });
            }
            Err(err) => {
                println!(
                    "  FAILED   {label}\n             {err}\n             → {}",
                    err.remedy()
                );
            }
        }
    }

    if tracks.is_empty() {
        eprintln!("\nNothing opened.");
        std::process::exit(1);
    }

    println!();
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(seconds) {
        std::thread::sleep(Duration::from_millis(200));

        let meters: Vec<String> = tracks
            .iter()
            .map(|track| {
                let level = track.session.level();
                let filled = (level.rms * 16.0).min(16.0) as usize;
                format!("[{:<16}]", "#".repeat(filled))
            })
            .collect();

        print!(
            "\r  {:>4.1}s  {}",
            started.elapsed().as_secs_f32(),
            meters.join(" ")
        );
        use std::io::Write;
        let _ = std::io::stdout().flush();
    }
    println!("\n");

    let mut alive = 0;
    for track in tracks {
        let failure = track.session.failure();
        let finished = match track.session.stop() {
            Ok(finished) => finished,
            Err(err) => {
                println!("  {:<34} stop failed: {err}", track.label);
                continue;
            }
        };

        let samples = read_mono(std::path::Path::new(&finished.path)).unwrap_or_default();
        let peak = samples.iter().fold(0.0f32, |max, s| max.max(s.abs()));

        println!(
            "  {:<34} {:>6} ms  {:>7} samples  peak {:.4}{}",
            track.label,
            finished.duration_ms,
            samples.len(),
            peak,
            match failure {
                Some(err) => format!("  ← stream failed: {err}"),
                None => String::new(),
            }
        );

        if peak > 0.001 {
            alive += 1;
        }
    }

    println!(
        "\n{alive} source(s) captured audio. Files are in {}",
        dir.display()
    );
}
