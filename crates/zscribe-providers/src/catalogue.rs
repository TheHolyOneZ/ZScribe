use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CatalogueEntry {
    pub id: &'static str,

    pub label: &'static str,

    #[ts(type = "number")]
    pub megabytes: u64,

    #[ts(type = "number")]
    pub overhead_mb: u64,

    pub summary: &'static str,
}

impl CatalogueEntry {
    pub fn required_mb(&self) -> u64 {
        self.megabytes + self.overhead_mb
    }
}

pub const CATALOGUE: [CatalogueEntry; 6] = [
    CatalogueEntry {
        id: "qwen2.5:1.5b",
        label: "Qwen2.5 1.5B",
        megabytes: 986,
        overhead_mb: 600,
        summary: "Runs anywhere. Fine for a short self-note, loses the thread on a meeting.",
    },
    CatalogueEntry {
        id: "llama3.2:3b",
        label: "Llama 3.2 3B",
        megabytes: 2_000,
        overhead_mb: 900,
        summary: "The smallest that writes a usable meeting summary.",
    },
    CatalogueEntry {
        id: "qwen2.5:7b",
        label: "Qwen2.5 7B",
        megabytes: 4_700,
        overhead_mb: 1_500,
        summary: "The sensible default. Good at structure and at following a template.",
    },
    CatalogueEntry {
        id: "gemma2:9b",
        label: "Gemma 2 9B",
        megabytes: 5_400,
        overhead_mb: 1_700,
        summary: "Reads long, rambling conversations better than most of its size.",
    },
    CatalogueEntry {
        id: "qwen2.5:14b",
        label: "Qwen2.5 14B",
        megabytes: 9_000,
        overhead_mb: 2_200,
        summary: "Where summaries stop needing a second look. KONZEPT.md's threshold.",
    },
    CatalogueEntry {
        id: "qwen2.5:32b",
        label: "Qwen2.5 32B",
        megabytes: 19_900,
        overhead_mb: 3_500,
        summary: "Better again, and only worth it with a large graphics card.",
    },
];

pub const PREFERRED: &str = "qwen2.5:7b";

pub fn find(id: &str) -> Option<&'static CatalogueEntry> {
    CATALOGUE.iter().find(|entry| entry.id == id)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Suggestion {
    pub model_id: String,

    pub headline: String,

    pub viable: Vec<String>,
}

pub fn suggest(vram_mb: u64, available_ram_mb: u64, gpu_name: Option<&str>) -> Suggestion {
    let budget = if vram_mb > 0 {
        vram_mb.saturating_sub(1_024)
    } else {
        available_ram_mb.saturating_sub(2_048)
    };

    let viable: Vec<&CatalogueEntry> = CATALOGUE
        .iter()
        .filter(|entry| entry.required_mb() <= budget)
        .collect();

    let chosen = viable
        .iter()
        .find(|entry| entry.id == PREFERRED)
        .copied()
        .or_else(|| viable.last().copied())
        .unwrap_or(&CATALOGUE[0]);

    let headline = match gpu_name {
        Some(name) if vram_mb > 0 => format!(
            "{} — {:.1} GB. Your {} ({} GB) holds this comfortably, so summaries take seconds \
             rather than minutes.",
            chosen.label,
            chosen.megabytes as f64 / 1024.0,
            name,
            (vram_mb as f64 / 1024.0).round() as u64,
        ),
        _ => format!(
            "{} — {:.1} GB. No graphics card is available to Ollama, so this runs on the \
             processor and a long recording will take a while.",
            chosen.label,
            chosen.megabytes as f64 / 1024.0,
        ),
    };

    Suggestion {
        model_id: chosen.id.to_owned(),
        headline,
        viable: viable.iter().map(|entry| entry.id.to_owned()).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_and_listed_smallest_first() {
        for pair in CATALOGUE.windows(2) {
            assert!(
                pair[0].megabytes < pair[1].megabytes,
                "{} vs {}",
                pair[0].id,
                pair[1].id
            );
        }

        let mut ids: Vec<&str> = CATALOGUE.iter().map(|e| e.id).collect();
        ids.sort_unstable();
        let total = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), total);
    }

    #[test]
    fn every_entry_is_completely_described() {
        for entry in CATALOGUE {
            assert!(!entry.label.is_empty(), "{}", entry.id);
            assert!(!entry.summary.is_empty(), "{}", entry.id);
            assert!(entry.megabytes > 0 && entry.overhead_mb > 0, "{}", entry.id);
            assert!(entry.id.contains(':'), "{} is not an ollama tag", entry.id);
        }
    }

    #[test]
    fn the_preferred_model_is_in_the_catalogue() {
        assert!(find(PREFERRED).is_some());
    }

    #[test]
    fn a_twelve_gigabyte_card_is_offered_the_sensible_default() {
        let suggestion = suggest(12_032, 19_000, Some("AMD Radeon RX 6700 XT"));

        assert_eq!(suggestion.model_id, "qwen2.5:7b");
        assert!(
            suggestion.headline.contains("RX 6700 XT"),
            "{}",
            suggestion.headline
        );
        assert!(
            suggestion.headline.contains("12 GB"),
            "{}",
            suggestion.headline
        );
    }

    #[test]
    fn a_large_card_still_gets_the_default_rather_than_the_biggest_thing_that_fits() {
        let suggestion = suggest(48_000, 64_000, Some("Big Card"));

        assert_eq!(suggestion.model_id, PREFERRED);
        assert!(suggestion.viable.contains(&"qwen2.5:32b".to_owned()));
    }

    #[test]
    fn a_small_card_is_offered_something_that_fits_in_it() {
        let suggestion = suggest(4_096, 16_000, Some("Modest Card"));

        let chosen = find(&suggestion.model_id).expect("in the catalogue");
        assert!(chosen.required_mb() <= 4_096, "{} does not fit", chosen.id);
    }

    #[test]
    fn no_graphics_card_falls_back_to_system_memory_and_says_so() {
        let suggestion = suggest(0, 16_000, None);

        assert!(
            suggestion.headline.contains("processor"),
            "{}",
            suggestion.headline
        );
        assert!(
            !suggestion.headline.contains("graphics card ("),
            "{}",
            suggestion.headline
        );
    }

    #[test]
    fn a_machine_with_almost_no_memory_still_gets_a_suggestion() {
        let suggestion = suggest(0, 1_000, None);
        assert!(find(&suggestion.model_id).is_some());
    }

    #[test]
    fn the_suggestion_is_among_the_viable_ones_when_any_are() {
        let suggestion = suggest(12_032, 19_000, Some("Card"));
        assert!(suggestion.viable.contains(&suggestion.model_id));
    }

    #[test]
    fn running_a_model_needs_more_memory_than_downloading_it() {
        for entry in CATALOGUE {
            assert!(entry.required_mb() > entry.megabytes, "{}", entry.id);
        }
    }
}
