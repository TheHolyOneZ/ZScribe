use tokio_util::sync::CancellationToken;
use zscribe_providers::{catalogue, ollama::Ollama};

#[tokio::main]
async fn main() {
    let model = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "qwen2.5:1.5b".to_owned());
    let ollama = Ollama::new("http://localhost:11434".to_owned()).expect("client");

    if let Some(entry) = catalogue::find(&model) {
        println!(
            "{} — {} MB\n  {}",
            entry.label, entry.megabytes, entry.summary
        );
    }

    println!("\nInstalling {model}");
    ollama
        .pull(&model, &CancellationToken::new(), |progress| {
            print!(
                "\r  {:>3}%  {} / {} MB   ",
                progress.percent,
                progress.downloaded_bytes / 1_048_576,
                progress.total_bytes / 1_048_576
            );
            use std::io::Write;
            let _ = std::io::stdout().flush();
        })
        .await
        .unwrap_or_else(|err| panic!("\n{err}\n  → {}", err.remedy()));

    println!("\r  installed                    ");

    println!("\nLoaded right now:");
    for loaded in ollama.running().await.expect("ps") {
        println!(
            "  {} — {:.1} GB, {}",
            loaded.model,
            loaded.size_bytes as f64 / 1_073_741_824.0,
            if loaded.on_gpu() {
                "on the graphics card"
            } else {
                "on the processor"
            }
        );
    }
}
