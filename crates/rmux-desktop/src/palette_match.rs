/// Lower scores rank first. Callers normalize case once for each title and query.
/// Each query word may match anywhere; contiguous matches beat subsequences.
pub fn score(title: &str, words: &[&str]) -> Option<(usize, usize, usize)> {
    let mut fuzzy = 0;
    let mut gaps = 0;
    let mut starts = 0;
    for word in words {
        if let Some(start) = title.find(word) {
            starts += start;
            continue;
        }
        fuzzy += 1;
        let mut chars = title.chars().enumerate();
        let mut previous = None;
        for wanted in word.chars() {
            let (position, _) = chars.find(|(_, ch)| *ch == wanted)?;
            if let Some(previous) = previous {
                gaps += position - previous - 1;
            } else {
                starts += position;
            }
            previous = Some(position);
        }
    }
    Some((fuzzy, gaps, starts))
}

/// Rank tab titles and directories independently for each word. Prefer title
/// matches of equal quality, without allowing a fuzzy word to cross fields.
pub fn tab_score(number: usize, title: &str, directory: &str, words: &[&str]) -> Option<(usize, usize, usize, usize)> {
    let mut total = (0, 0, 0, 0);
    let number = number.to_string();
    for word in words {
        // Displayed tab numbers are exact tokens, not fuzzy substrings: 3
        // should target tab 3, rather than also matching tab 13 or tab 30.
        if *word == number { continue; }
        let title_score = score(title, &[*word]).map(|(fuzzy, gaps, start)| (fuzzy, gaps, 1, start));
        let directory_score = score(directory, &[*word]).map(|(fuzzy, gaps, start)| (fuzzy, gaps, 2, start));
        let best = match (title_score, directory_score) {
            (Some(a), Some(b)) => a.min(b),
            (Some(a), None) | (None, Some(a)) => a,
            (None, None) => return None,
        };
        total.0 += best.0;
        total.1 += best.1;
        total.2 += best.2;
        total.3 += best.3;
    }
    Some(total)
}

#[cfg(test)]
mod tests {
    use super::{score, tab_score};

    #[test]
    fn tab_numbers_are_exact_tokens_and_combine_with_metadata() {
        assert!(tab_score(3, "server", "/work", &["3", "srv"]).is_some());
        assert!(tab_score(13, "server", "/work", &["3"]).is_none());
        assert!(tab_score(3, "server", "/work", &["03"]).is_none());
        assert!(tab_score(3, "server", "/work", &["3"]) < tab_score(1, "server3", "", &["3"]));
        assert!(tab_score(1, "server3", "", &["3"]).is_some());
    }

    #[test]
    fn tab_search_combines_fields_without_crossing_them() {
        assert!(tab_score(1, "build server", "/work/rmux", &["bld", "rmx"]).is_some());
        assert!(tab_score(1, "alpha", "/beta", &["ab"]).is_none());
        assert!(tab_score(1, "東京", "/work/café", &["東", "café"]).is_some());
        assert!(tab_score(1, "rmux", "/other", &["rmux"]) < tab_score(1, "other", "/rmux", &["rmux"]));
        assert!(tab_score(1, "r m u x", "", &["rmux"]) > tab_score(1, "other", "/rmux", &["rmux"]));
        assert_eq!(tab_score(1, "anything", "/anywhere", &[]), Some((0, 0, 0, 0)));
    }

    #[test]
    fn abbreviations_rank_after_contiguous_matches() {
        let words = ["splt", "blw"];
        assert!(score("split below", &words).is_some());
        assert!(score("split right", &words).is_none());
        assert!(score("splt blw", &words) < score("split below", &words));
        assert!(
            score("split below", &["split"]).unwrap() < score("s p l i t", &["split"]).unwrap()
        );
    }

    #[test]
    fn words_match_independently_and_unicode_is_not_byte_sliced() {
        assert!(score("switch to workspace: 東京 café", &["café", "東"]).is_some());
        assert!(score("東京作業", &["東業"]).is_some());
        assert!(score("東京作業", &["業東"]).is_none());
        assert!(score("tab", &["tt"]).is_none());
        assert_eq!(score("anything", &[]), Some((0, 0, 0)));
    }
}
