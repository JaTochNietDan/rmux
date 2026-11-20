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
pub fn tab_score(title: &str, directory: &str, words: &[&str]) -> Option<(usize, usize, usize, usize)> {
    let mut total = (0, 0, 0, 0);
    for word in words {
        let title_score = score(title, &[*word]).map(|(fuzzy, gaps, start)| (fuzzy, gaps, 0, start));
        let directory_score = score(directory, &[*word]).map(|(fuzzy, gaps, start)| (fuzzy, gaps, 1, start));
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
    fn tab_search_combines_fields_without_crossing_them() {
        assert!(tab_score("build server", "/work/rmux", &["bld", "rmx"]).is_some());
        assert!(tab_score("alpha", "/beta", &["ab"]).is_none());
        assert!(tab_score("東京", "/work/café", &["東", "café"]).is_some());
        assert!(tab_score("rmux", "/other", &["rmux"]) < tab_score("other", "/rmux", &["rmux"]));
        assert!(tab_score("r m u x", "", &["rmux"]) > tab_score("other", "/rmux", &["rmux"]));
        assert_eq!(tab_score("anything", "/anywhere", &[]), Some((0, 0, 0, 0)));
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
