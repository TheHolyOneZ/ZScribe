#![forbid(unsafe_code)]

pub mod advisor;
pub mod download;
pub mod models;
pub mod speakers;
pub mod transcribe;

pub use advisor::{recommend, Recommendation};
pub use download::{fetch, installed, DownloadError, InstalledModel, Progress};
pub use models::{find, ModelSpec, MODELS, PREFERRED_MODEL_ID};
pub use speakers::{label as label_speakers, Heard};
pub use transcribe::{
    gpu_support_compiled_in, options_for, transcribe, transcribe_file, Options, SttError,
};
