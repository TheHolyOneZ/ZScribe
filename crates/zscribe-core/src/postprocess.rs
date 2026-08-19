pub fn clean_model_output(raw: &str) -> String {
    let text = raw.trim();
    let text = strip_code_fence(text);
    let text = strip_preamble(text);
    text.trim().to_owned()
}

fn strip_code_fence(text: &str) -> &str {
    let Some(rest) = text.strip_prefix("```") else {
        return text;
    };

    let Some((_tag, body)) = rest.split_once('\n') else {
        return text;
    };

    let Some(inner) = body.trim_end().strip_suffix("```") else {
        return text;
    };

    if inner.contains("```") {
        return text;
    }

    inner
}

fn strip_preamble(text: &str) -> &str {
    let Some((first, rest)) = text.split_once('\n') else {
        return text;
    };
    let first = first.trim();

    let is_preamble = first.ends_with(':')
        && first.len() <= 80
        && !first.starts_with('#')
        && !first.starts_with('-')
        && !first.starts_with('*')
        && !first.starts_with('|');

    if is_preamble {
        rest.trim_start()
    } else {
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_output_is_returned_untouched() {
        assert_eq!(
            clean_model_output("## Decisions\n\n- Ship it"),
            "## Decisions\n\n- Ship it"
        );
    }

    #[test]
    fn a_wrapping_fence_is_removed() {
        assert_eq!(clean_model_output("```markdown\n# Notes\n```"), "# Notes");
        assert_eq!(clean_model_output("```\n# Notes\n```"), "# Notes");
    }

    #[test]
    fn a_code_block_inside_a_summary_survives() {
        let body = "# Notes\n\n```rust\nfn main() {}\n```\n\nDone.";
        assert_eq!(clean_model_output(body), body);
    }

    #[test]
    fn a_conversational_preamble_is_dropped() {
        assert_eq!(
            clean_model_output("Here is the summary of your meeting:\n\n# Decisions"),
            "# Decisions"
        );
    }

    #[test]
    fn a_markdown_heading_is_never_mistaken_for_a_preamble() {
        let body = "# Meeting: Q3 planning\n\n- Agreed the date";
        assert_eq!(clean_model_output(body), body);
    }

    #[test]
    fn a_list_item_ending_in_a_colon_is_not_a_preamble() {
        let body = "- Decision:\n- Another";
        assert_eq!(clean_model_output(body), body);
    }

    #[test]
    fn a_long_first_line_is_content_not_a_preamble() {
        let body = format!("{}:\n\nrest", "word ".repeat(30));
        assert_eq!(clean_model_output(&body), body.trim());
    }

    #[test]
    fn surrounding_whitespace_goes_away() {
        assert_eq!(clean_model_output("\n\n  # Notes  \n\n"), "# Notes");
    }

    #[test]
    fn an_empty_response_stays_empty_rather_than_panicking() {
        assert_eq!(clean_model_output(""), "");
        assert_eq!(clean_model_output("```"), "```");
    }
}
