use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Instant;

use zscribe_platform::Machine;
use zscribe_stt::{advisor, download, models, transcribe};

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(audio_path) = args.next() else {
        eprintln!("usage: try <file.wav> [model-id]");
        std::process::exit(2);
    };

    let models_dir = PathBuf::from(
        std::env::var("ZSCRIBE_MODELS_DIR").unwrap_or_else(|_| "/tmp/zscribe-models".to_owned()),
    );
    std::fs::create_dir_all(&models_dir).expect("models dir");

    let machine = Machine::probe(&models_dir);
    let recommendation = advisor::recommend(&machine);

    println!("Scan");
    println!("  {}", recommendation.headline);
    for note in &recommendation.notes {
        println!("  [{:?}] {} — {}", note.severity, note.title, note.detail);
    }

    let model_id = args
        .next()
        .unwrap_or_else(|| recommendation.model_id.clone());
    let spec = models::find(&model_id).expect("known model");
    println!("\nModel {} ({} MB)", spec.label, spec.megabytes());

    let started = Instant::now();
    let model_path = download::fetch(
        &model_id,
        &models_dir,
        machine.free_disk_mb,
        Arc::new(AtomicBool::new(false)),
        |progress| {
            if progress.verifying {
                print!("\r  verifying checksum…            ");
            } else {
                print!(
                    "\r  {:>3}%  {:>6} / {} MB   ",
                    progress.percent,
                    progress.downloaded_bytes / 1_048_576,
                    progress.total_bytes / 1_048_576
                );
            }
            use std::io::Write;
            let _ = std::io::stdout().flush();
        },
    )
    .unwrap_or_else(|err| {
        eprintln!("\ndownload failed: {err}\n  → {}", err.remedy());
        std::process::exit(1);
    });
    println!(
        "\r  ready in {:.1}s                        ",
        started.elapsed().as_secs_f32()
    );

    let options = transcribe::Options {
        model_path,
        model_id: model_id.clone(),
        language: None,
        threads: machine.whisper_threads(),
        use_gpu: machine.can_accelerate(),
    };

    println!(
        "\nTranscribing {audio_path} ({} threads, gpu {})",
        options.threads, options.use_gpu
    );

    let started = Instant::now();
    let transcript = transcribe::transcribe_file(
        Path::new(&audio_path),
        &options,
        Arc::new(AtomicBool::new(false)),
        |percent| {
            print!("\r  {percent:>3}%");
            use std::io::Write;
            let _ = std::io::stdout().flush();
        },
    )
    .unwrap_or_else(|err| {
        eprintln!("\ntranscription failed: {err}\n  → {}", err.remedy());
        std::process::exit(1);
    });

    let elapsed = started.elapsed();
    let audio_seconds = transcript.duration_ms() as f32 / 1000.0;

    println!("\r  done in {:.1}s", elapsed.as_secs_f32());
    println!("\nLanguage  {}", transcript.language);
    println!("Segments  {}", transcript.segments.len());
    if elapsed.as_secs_f32() > 0.0 {
        println!(
            "Speed     {:.1}x real time",
            audio_seconds / elapsed.as_secs_f32()
        );
    }

    println!("\n--- transcript ---");
    for segment in &transcript.segments {
        println!(
            "[{}] {}",
            zscribe_core::format_offset(segment.start_ms),
            segment.text
        );
    }
}
