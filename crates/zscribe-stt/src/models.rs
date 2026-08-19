use serde::{Deserialize, Serialize};
use std::path::Path;
use ts_rs::TS;

const BASE_URL: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ModelSpec {
    pub id: &'static str,

    pub label: &'static str,

    #[ts(type = "number")]
    pub bytes: u64,

    #[ts(type = "number")]
    pub runtime_mb: u64,

    pub gpu_speed: u32,

    pub cpu_speed: u32,

    pub summary: &'static str,

    pub sha256: &'static str,
}

impl ModelSpec {
    pub fn url(&self) -> String {
        format!("{BASE_URL}/{}", self.file_name())
    }

    pub fn file_name(&self) -> String {
        format!("ggml-{}.bin", self.id)
    }

    pub fn megabytes(&self) -> u64 {
        self.bytes / (1024 * 1024)
    }

    pub fn required_mb(&self) -> u64 {
        self.megabytes() + self.runtime_mb
    }

    pub fn looks_installed(&self, path: &Path) -> bool {
        std::fs::metadata(path).is_ok_and(|meta| meta.len() == self.bytes)
    }
}

pub const MODELS: [ModelSpec; 6] = [
    ModelSpec {
        id: "tiny",
        label: "Tiny",
        bytes: 77_691_713,
        runtime_mb: 130,
        gpu_speed: 60,
        cpu_speed: 12,
        summary: "Keywords and rough notes. Misses names and numbers.",
        sha256: "be07e048e1e599ad46341c8d2a135645097a538221678b7acdd1b1919c6e1b21",
    },
    ModelSpec {
        id: "base",
        label: "Base",
        bytes: 147_951_465,
        runtime_mb: 210,
        gpu_speed: 40,
        cpu_speed: 8,
        summary: "Usable on a weak machine. Still loose with proper nouns.",
        sha256: "60ed5bc3dd14eea856493d334349b405782ddcaf0028d4b5df4088345fba2efe",
    },
    ModelSpec {
        id: "small",
        label: "Small",
        bytes: 487_601_967,
        runtime_mb: 600,
        gpu_speed: 25,
        cpu_speed: 3,
        summary: "The sensible floor for real conversations.",
        sha256: "1be3a9b2063867b937e64e2ec7483364a79917e157fa98c5d94b5c1fffea987b",
    },
    ModelSpec {
        id: "medium",
        label: "Medium",
        bytes: 1_533_763_059,
        runtime_mb: 1_700,
        gpu_speed: 12,
        cpu_speed: 1,
        summary: "Accurate, and slow without a GPU.",
        sha256: "6c14d5adee5f86394037b4e4e8b59f1673b6cee10e3cf0b11bbdbee79c156208",
    },
    ModelSpec {
        id: "large-v3-turbo",
        label: "Large v3 Turbo",
        bytes: 1_624_555_275,
        runtime_mb: 1_800,
        gpu_speed: 20,
        cpu_speed: 2,
        summary: "Large-model accuracy at close to medium speed. The one to want.",
        sha256: "1fc70f774d38eb169993ac391eea357ef47c88757ef72ee5943879b7e8e2bc69",
    },
    ModelSpec {
        id: "large-v3",
        label: "Large v3",
        bytes: 3_095_033_483,
        runtime_mb: 3_400,
        gpu_speed: 8,
        cpu_speed: 1,
        summary: "The most accurate, and the slowest. Rarely worth it over Turbo.",
        sha256: "64d182b440b98d5203c4f9bd541544d84c605196c4f7b845dfa11fb23594d1e2",
    },
];

pub const PREFERRED_MODEL_ID: &str = "large-v3-turbo";

pub fn find(id: &str) -> Option<&'static ModelSpec> {
    MODELS.iter().find(|model| model.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_ids_are_unique() {
        let mut ids: Vec<&str> = MODELS.iter().map(|m| m.id).collect();
        ids.sort_unstable();
        let total = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), total);
    }

    #[test]
    fn the_preferred_model_exists() {
        assert!(find(PREFERRED_MODEL_ID).is_some());
    }

    #[test]
    fn every_model_is_completely_described() {
        for model in MODELS {
            assert!(!model.label.is_empty(), "{}", model.id);
            assert!(!model.summary.is_empty(), "{}", model.id);
            assert!(model.bytes > 0, "{}", model.id);
            assert!(model.runtime_mb > 0, "{}", model.id);
            assert!(model.gpu_speed > 0 && model.cpu_speed > 0, "{}", model.id);
        }
    }

    #[test]
    fn every_checksum_is_a_real_sha256() {
        for model in MODELS {
            assert_eq!(model.sha256.len(), 64, "{}", model.id);
            assert!(
                model
                    .sha256
                    .chars()
                    .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
                "{} has a checksum that is not lowercase hex",
                model.id
            );
        }

        let mut hashes: Vec<&str> = MODELS.iter().map(|m| m.sha256).collect();
        hashes.sort_unstable();
        let total = hashes.len();
        hashes.dedup();
        assert_eq!(hashes.len(), total, "two models cannot share a checksum");
    }

    #[test]
    fn models_are_listed_smallest_first() {
        for pair in MODELS.windows(2) {
            assert!(
                pair[0].bytes < pair[1].bytes,
                "{} is not smaller than {}",
                pair[0].id,
                pair[1].id
            );
        }
    }

    #[test]
    fn urls_point_at_the_published_ggml_files() {
        let turbo = find("large-v3-turbo").expect("present");
        assert_eq!(
            turbo.url(),
            "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo.bin"
        );
        assert!(turbo.url().starts_with("https://"), "never plain HTTP");
    }

    #[test]
    fn file_names_match_what_whisper_cpp_expects() {
        assert_eq!(
            find("small").expect("present").file_name(),
            "ggml-small.bin"
        );
    }

    #[test]
    fn reported_sizes_are_close_to_the_advertised_round_numbers() {
        assert_eq!(find("tiny").expect("present").megabytes(), 74);
        assert_eq!(find("small").expect("present").megabytes(), 465);
        assert_eq!(find("large-v3-turbo").expect("present").megabytes(), 1549);
    }

    #[test]
    fn running_a_model_needs_more_memory_than_storing_it() {
        for model in MODELS {
            assert!(model.required_mb() > model.megabytes(), "{}", model.id);
        }
    }

    #[test]
    fn turbo_is_faster_than_the_full_large_model_it_replaces() {
        let turbo = find("large-v3-turbo").expect("present");
        let large = find("large-v3").expect("present");
        assert!(turbo.gpu_speed > large.gpu_speed);
        assert!(turbo.bytes < large.bytes);
    }

    #[test]
    fn a_missing_file_is_not_mistaken_for_an_installed_model() {
        let model = find("tiny").expect("present");
        assert!(!model.looks_installed(Path::new("/nonexistent/ggml-tiny.bin")));
    }

    #[test]
    fn a_half_finished_download_is_not_mistaken_for_an_installed_model() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("ggml-tiny.bin");
        std::fs::write(&path, b"only the first few bytes").expect("write");

        assert!(!find("tiny").expect("present").looks_installed(&path));
    }

    #[test]
    fn an_unknown_id_is_not_found_rather_than_defaulting() {
        assert!(find("whisper-nine").is_none());
        assert!(find("").is_none());
    }
}
