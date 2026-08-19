use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::transcript::{format_offset, Transcript};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TokenUsage {
    pub input: u32,
    pub output: u32,
}

impl TokenUsage {
    pub fn total(self) -> u32 {
        self.input.saturating_add(self.output)
    }

    pub fn merge(self, other: Self) -> Self {
        Self {
            input: self.input.saturating_add(other.input),
            output: self.output.saturating_add(other.output),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ActionItem {
    pub task: String,
    pub owner: Option<String>,
    pub due: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Summary {
    pub provider: String,
    pub model: String,
    pub template_id: String,

    pub body_md: String,

    pub action_items: Vec<ActionItem>,
    pub usage: TokenUsage,
    pub elapsed_ms: u32,

    #[serde(default)]
    pub redacted: u32,
}

pub fn extract_action_items(body_md: &str) -> Vec<ActionItem> {
    let Some(section) = action_section(body_md) else {
        return Vec::new();
    };

    let rows: Vec<ActionItem> = section
        .lines()
        .filter_map(parse_table_row)
        .chain(section.lines().filter_map(parse_bullet))
        .filter(|item| !item.task.is_empty())
        .collect();

    rows
}

fn action_section(body_md: &str) -> Option<&str> {
    let mut lines = body_md.lines();
    let mut offset = 0;

    let (level, start) = loop {
        let line = lines.next()?;
        let consumed = line.len() + 1;

        if let Some(level) = heading_level(line) {
            let title = line.trim_start_matches('#').trim().to_lowercase();
            if title.contains("action") || title.contains("next step") || title.contains("task") {
                break (level, offset + consumed);
            }
        }
        offset += consumed;
    };

    let mut end = body_md.len();
    let mut cursor = start;
    for line in body_md.get(start..)?.lines() {
        if heading_level(line).is_some_and(|found| found <= level) {
            end = cursor;
            break;
        }
        cursor += line.len() + 1;
    }

    body_md.get(start..end.min(body_md.len()))
}

fn heading_level(line: &str) -> Option<usize> {
    let hashes = line.chars().take_while(|c| *c == '#').count();
    (1..=6).contains(&hashes).then_some(hashes)
}

fn parse_table_row(line: &str) -> Option<ActionItem> {
    let line = line.trim();
    if !line.starts_with('|') {
        return None;
    }

    let cells: Vec<&str> = line.trim_matches('|').split('|').map(str::trim).collect();
    if cells.len() < 2 {
        return None;
    }

    if cells
        .iter()
        .all(|c| c.chars().all(|ch| ch == '-' || ch == ':') && !c.is_empty())
    {
        return None;
    }
    if cells[0].eq_ignore_ascii_case("task") {
        return None;
    }

    Some(ActionItem {
        task: cells[0].to_owned(),
        owner: meaningful(cells.get(1).copied()),
        due: meaningful(cells.get(2).copied()),
    })
}

fn parse_bullet(line: &str) -> Option<ActionItem> {
    let line = line.trim();
    let rest = line
        .strip_prefix("- ")
        .or_else(|| line.strip_prefix("* "))
        .or_else(|| {
            line.split_once(". ")
                .filter(|(n, _)| n.chars().all(|c| c.is_ascii_digit()))
                .map(|(_, rest)| rest)
        })?;

    let rest = rest
        .trim()
        .trim_start_matches("[ ]")
        .trim_start_matches("[x]")
        .trim();
    if rest.is_empty() {
        return None;
    }

    Some(ActionItem {
        task: rest.trim_end_matches('.').to_owned(),
        owner: None,
        due: None,
    })
}

fn meaningful(cell: Option<&str>) -> Option<String> {
    let cell = cell?.trim();
    let empty = cell.is_empty()
        || cell == "-"
        || cell == "—"
        || cell.eq_ignore_ascii_case("none")
        || cell.eq_ignore_ascii_case("n/a")
        || cell.eq_ignore_ascii_case("unassigned")
        || cell.eq_ignore_ascii_case("owner")
        || cell.eq_ignore_ascii_case("due");
    (!empty).then(|| cell.to_owned())
}

pub fn to_markdown(
    title: &str,
    recorded_at: &str,
    summary: Option<&Summary>,
    transcript: Option<&Transcript>,
) -> String {
    let mut out = format!("# {title}\n\n_{recorded_at}_\n");

    if let Some(summary) = summary {
        out.push_str(&format!("\n{}\n", summary.body_md.trim()));
    }

    if let Some(transcript) = transcript {
        out.push_str("\n## Transcript\n\n");
        for segment in &transcript.segments {
            let text = segment.text.trim();
            if text.is_empty() {
                continue;
            }
            out.push_str(&format!(
                "**{}** {text}\n\n",
                format_offset(segment.start_ms)
            ));
        }
    }

    if let Some(summary) = summary {
        out.push_str(&format!(
            "\n---\n\nSummarised by {} ({}).\n",
            summary.model, summary.provider
        ));
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcript::Segment;

    #[test]
    fn token_usage_adds_across_a_map_reduce_run() {
        let total = TokenUsage {
            input: 10,
            output: 5,
        }
        .merge(TokenUsage {
            input: 3,
            output: 2,
        });
        assert_eq!(
            total,
            TokenUsage {
                input: 13,
                output: 7
            }
        );
        assert_eq!(total.total(), 20);
    }

    #[test]
    fn a_table_of_action_items_is_read() {
        let body = "\
## Action items

| Task | Owner | Due |
|---|---|---|
| Send the contract | Anna | Friday |
| Book the room | Ben | none |
";
        let items = extract_action_items(body);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].task, "Send the contract");
        assert_eq!(items[0].owner.as_deref(), Some("Anna"));
        assert_eq!(items[0].due.as_deref(), Some("Friday"));

        assert_eq!(items[1].due, None);
    }

    #[test]
    fn the_header_row_and_the_rule_are_not_action_items() {
        let body =
            "## Action items\n\n| Task | Owner | Due |\n|---|---|---|\n| Do it | Anna | - |\n";
        assert_eq!(extract_action_items(body).len(), 1);
    }

    #[test]
    fn a_bullet_list_is_read_when_the_model_ignores_the_table() {
        let body = "## Next steps\n\n- Send the contract\n- Book the room\n";
        let items = extract_action_items(body);
        assert_eq!(items.len(), 2);
        assert_eq!(items[1].task, "Book the room");
    }

    #[test]
    fn checkbox_and_numbered_lists_are_read_too() {
        let body = "## Action items\n\n- [ ] Send the contract\n2. Book the room\n";
        let items = extract_action_items(body);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].task, "Send the contract");
        assert_eq!(items[1].task, "Book the room");
    }

    #[test]
    fn only_the_action_section_is_scanned() {
        let body = "\
## Decisions

- We will ship on Friday

## Action items

- Send the contract

## Open points

- Nobody asked about pricing
";
        let items = extract_action_items(body);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].task, "Send the contract");
    }

    #[test]
    fn a_deeper_subheading_stays_inside_the_section() {
        let body =
            "## Action items\n\n### This week\n\n- Send the contract\n\n## Notes\n\n- ignored\n";
        let items = extract_action_items(body);
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn a_summary_with_no_action_section_yields_nothing_rather_than_guessing() {
        let body = "## Summary\n\n- Talked about the weather\n- Talked about the roadmap\n";
        assert!(extract_action_items(body).is_empty());
    }

    #[test]
    fn the_nothing_recorded_wording_does_not_become_a_task() {
        let body = "## Action items\n\nNothing recorded\n";
        assert!(extract_action_items(body).is_empty());
    }

    #[test]
    fn markdown_export_carries_summary_transcript_and_attribution() {
        let summary = Summary {
            provider: "Ollama (local)".to_owned(),
            model: "qwen2.5:7b".to_owned(),
            template_id: "meeting".to_owned(),
            body_md: "## Decisions\n\n- Ship it".to_owned(),
            action_items: Vec::new(),
            usage: TokenUsage::default(),
            elapsed_ms: 1200,
            redacted: 0,
        };
        let transcript = Transcript {
            language: "en".to_owned(),
            model: "large-v3-turbo".to_owned(),
            segments: vec![Segment::new(72_000, 74_000, "Let us ship it.")],
        };

        let md = to_markdown(
            "Planning call",
            "7 August 2026, 14:02",
            Some(&summary),
            Some(&transcript),
        );

        assert!(md.starts_with("# Planning call"));
        assert!(md.contains("## Decisions"));
        assert!(md.contains("**1:12** Let us ship it."));
        assert!(md.contains("qwen2.5:7b"));
    }

    #[test]
    fn export_works_before_the_summary_exists() {
        let md = to_markdown("Untitled", "today", None, None);
        assert!(md.contains("# Untitled"));
        assert!(!md.contains("Summarised by"));
    }
}
