#![forbid(unsafe_code)]

pub mod archive;
pub mod chat;
pub mod diarize;
pub mod postprocess;
pub mod prompt;
pub mod provider;
pub mod redact;
pub mod subtitles;
pub mod summary;
pub mod template;
pub mod transcript;
pub mod voices;

pub use archive::{passages, similarity, Passage};
pub use chat::{Role, Turn};
pub use diarize::{merge as merge_tracks, speakers, Track};
pub use postprocess::clean_model_output;
pub use prompt::{Options as PromptOptions, Plan, Prompt, ReduceSpec, DEFAULT_CHUNK_CHARS};
pub use provider::{ModelPricing, ProviderId, ProviderProfile};
pub use redact::{redact, redact_transcript, Redaction, Removed};
pub use subtitles::{write as write_subtitles, Subtitles};
pub use summary::{ActionItem, Summary, TokenUsage};
pub use template::{builtin_template, builtin_templates, Template, DEFAULT_TEMPLATE_ID};
pub use transcript::{format_offset, Segment, Transcript};
pub use voices::{cluster as cluster_voices, Utterance, VoiceOptions};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
