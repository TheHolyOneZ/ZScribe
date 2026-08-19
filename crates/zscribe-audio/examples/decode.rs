use std::path::PathBuf;

fn main() {
    let mut args = std::env::args().skip(1);

    let Some(source) = args.next().map(PathBuf::from) else {
        eprintln!("usage: decode <file> [out.wav]");
        std::process::exit(2);
    };

    let destination = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| source.with_extension("decoded.wav"));

    match zscribe_audio::inspect(&source) {
        Ok(info) => println!(
            "  codec     {}\n  duration  {}",
            info.codec,
            info.duration_ms
                .map(|ms| format!("{:.1}s", ms as f32 / 1000.0))
                .unwrap_or_else(|| "not stated".to_owned()),
        ),
        Err(err) => {
            eprintln!("  cannot read it: {err}\n  {}", err.remedy());
            std::process::exit(1);
        }
    }

    let started = std::time::Instant::now();
    let mut last = 0;

    match zscribe_audio::decode_to_wav(&source, &destination, |percent| {
        if percent >= last + 10 {
            last = percent;
            print!("\r  decoding  {percent}%");
            use std::io::Write;
            let _ = std::io::stdout().flush();
        }
    }) {
        Ok(duration_ms) => {
            println!(
                "\r  decoded   {:.1}s of audio in {:.2}s -> {}",
                duration_ms as f32 / 1000.0,
                started.elapsed().as_secs_f32(),
                destination.display()
            );
        }
        Err(err) => {
            eprintln!("\r  failed: {err}\n  {}", err.remedy());
            std::process::exit(1);
        }
    }
}
