use tokio_util::sync::CancellationToken;
use zscribe_core::chat::{self, Context};
use zscribe_core::{ProviderId, ProviderProfile, Segment, Transcript, Turn};
use zscribe_providers::{build, CompletionRequest};

fn transcript() -> Transcript {
    let lines = [
        (0, "Right, let us start. The Q3 launch date.", "Anna Weiss"),
        (
            6_000,
            "I would say the fourteenth of October.",
            "Anna Weiss",
        ),
        (
            12_000,
            "That is tight. The contract is not signed yet.",
            "Max Kruger",
        ),
        (19_000, "I can send it to legal today.", "Max Kruger"),
        (25_000, "Do that. And we hold the fourteenth.", "Anna Weiss"),
        (
            31_000,
            "Agreed. I will have it back by Friday.",
            "Max Kruger",
        ),
        (
            38_000,
            "One more thing — the budget is unchanged.",
            "Anna Weiss",
        ),
    ];

    Transcript {
        language: "en".to_owned(),
        model: "large-v3-turbo".to_owned(),
        segments: lines
            .into_iter()
            .map(|(start, text, who)| Segment::new(start, start + 5_000, text).by(who))
            .collect(),
    }
}

#[tokio::main]
async fn main() {
    let transcript = transcript();
    let context = Context {
        transcript: &transcript,
        summary: None,
        title: "Q3 planning",
        timestamps: true,
    };

    let mut profile = ProviderProfile::new(ProviderId::Ollama);
    profile.model = std::env::var("ZSCRIBE_MODEL").unwrap_or_else(|_| "qwen2.5:7b".to_owned());

    let provider = build(&profile, None).expect("ollama builds without a key");
    let cancel = CancellationToken::new();

    let questions = [
        "What date was agreed for the launch?",
        "Who is sending the contract to legal?",
        "And when did they say it would be back?",
        "What is the budget in euros?",
        "Who else was in the meeting?",
    ];

    let mut history: Vec<Turn> = Vec::new();

    for question in questions {
        let prompt = chat::prompt(&context, &history, question);

        let completion = provider
            .complete(
                &CompletionRequest::new(&profile.model, prompt).with_history(history.clone()),
                &cancel,
            )
            .await
            .unwrap_or_else(|err| panic!("{question}: {err}\n  → {}", err.remedy()));

        let answer = zscribe_core::clean_model_output(&completion.text);

        println!("Q  {question}");
        println!("A  {}\n", answer.replace('\n', "\n   "));

        history.push(Turn::user(question));
        history.push(Turn::assistant(answer));
    }
}
