use std::time::Instant;
use tokio_util::sync::CancellationToken;
use zscribe_providers::ollama::Ollama;

#[tokio::main]
async fn main() {
    let model = std::env::args().nth(1).unwrap_or("qwen2.5:0.5b".to_owned());
    let ollama = Ollama::new("http://127.0.0.1:11434".to_owned()).expect("client");
    let started = Instant::now();
    let mut updates = 0u32;

    let result = ollama
        .pull(&model, &CancellationToken::new(), |progress| {
            updates += 1;
            if updates <= 6 || progress.percent % 25 == 0 || progress.total_bytes == 0 {
                println!(
                    "  {:>6.2}s  #{:<4} {:>3}%  {:>12}  {}",
                    started.elapsed().as_secs_f32(),
                    updates,
                    progress.percent,
                    progress.total_bytes,
                    progress.stage,
                );
            }
        })
        .await;

    println!(
        "\n  {updates} updates in {:.1}s — {result:?}",
        started.elapsed().as_secs_f32()
    );
}
