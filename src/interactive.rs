//! Shared helpers for interactive (`inquire`) prompts.

/// Case-insensitive substring scorer for `inquire` `MultiSelect` / `Select`
/// prompts.
///
/// `inquire`'s default scorer is a fuzzy matcher; this provides plain
/// case-insensitive *substring* matching instead. The query is matched against
/// the option's displayed text after stripping ANSI styling, so colored labels
/// (e.g. a cyan `[skill]` prefix) don't interfere with matching.
///
/// An empty query keeps every option. The returned score preserves the caller's
/// original option ordering: `inquire` sorts matches by `Reverse(score)` using an
/// unstable sort, so a score of `-(index)` keeps lower indices first.
pub fn substring_score(input: &str, string_value: &str, index: usize) -> Option<i64> {
    let matches = input.is_empty()
        || console::strip_ansi_codes(string_value)
            .to_lowercase()
            .contains(&input.to_lowercase());

    matches.then_some(-(index as i64))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_query_keeps_all_in_original_order() {
        // Every option is kept, and the score decreases with index so the
        // caller's ordering is preserved after inquire's Reverse(score) sort.
        assert_eq!(substring_score("", "anything", 0), Some(0));
        assert_eq!(substring_score("", "anything", 3), Some(-3));
        assert!(substring_score("", "x", 0) > substring_score("", "x", 1));
    }

    #[test]
    fn case_insensitive_substring_match() {
        assert_eq!(substring_score("HOME", "homelab", 0), Some(0));
        assert_eq!(substring_score("lab", "homelab", 5), Some(-5));
        assert_eq!(substring_score("git-commit", "git-commit", 0), Some(0));
    }

    #[test]
    fn non_matching_query_is_filtered_out() {
        assert_eq!(substring_score("xyz", "homelab", 0), None);
        assert_eq!(substring_score("commit", "pdf-reader", 0), None);
    }

    #[test]
    fn ansi_styling_is_ignored() {
        // "[skill] homelab (repo)" with ANSI color around the label.
        let styled = format!("\u{1b}[36m[skill]\u{1b}[0m homelab (github.com/o/r)");
        assert_eq!(substring_score("homelab", &styled, 0), Some(0));
        assert_eq!(substring_score("skill", &styled, 0), Some(0));
        assert_eq!(substring_score("github.com", &styled, 0), Some(0));
    }
}
