use std::path::PathBuf;

use zscribe_core::voices::VoiceOptions;
use zscribe_core::{Segment, Transcript};

fn main() {
    let mut args = std::env::args().skip(1);

    let Some(path) = args.next().map(PathBuf::from) else {
        eprintln!("usage: voices <file.wav> [start:end ...]");
        std::process::exit(2);
    };

    let ranges: Vec<(u32, u32)> = args
        .filter_map(|arg| {
            let (start, end) = arg.split_once(':')?;
            Some((
                (start.parse::<f32>().ok()? * 1000.0) as u32,
                (end.parse::<f32>().ok()? * 1000.0) as u32,
            ))
        })
        .collect();

    let ranges = if ranges.is_empty() {
        let samples = zscribe_audio::read_mono(&path).expect("read");
        let length = (samples.len() as f32 / zscribe_audio::SAMPLE_RATE as f32 * 1000.0) as u32;
        (0..length / 2_000)
            .map(|n| (n * 2_000, (n + 1) * 2_000))
            .collect()
    } else {
        ranges
    };

    let mut transcript = Transcript {
        language: "en".to_owned(),
        model: "none".to_owned(),
        segments: ranges
            .iter()
            .map(|(start_ms, end_ms)| Segment {
                start_ms: *start_ms,
                end_ms: *end_ms,
                text: String::new(),
                speaker: None,
            })
            .collect(),
    };

    let heard = zscribe_stt::label_speakers(&path, &mut transcript, &VoiceOptions::default());

    println!(
        "{} speakers, {} unattributed\n",
        heard.speakers, heard.unattributed
    );
    for segment in &transcript.segments {
        println!(
            "  {:>6.1}s → {:>6.1}s   {}",
            segment.start_ms as f32 / 1000.0,
            segment.end_ms as f32 / 1000.0,
            segment.speaker.as_deref().unwrap_or("—"),
        );
    }
}
