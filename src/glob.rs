//! Path globs over node ids: `*` matches within one `/`-separated segment,
//! `**` matches any number of whole segments (including none).

/// Whether `path` matches `pattern`.
pub fn matches(pattern: &str, path: &str) -> bool {
    let pat: Vec<&str> = pattern.split('/').filter(|s| !s.is_empty()).collect();
    let segs: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    match_segments(&pat, &segs)
}

fn match_segments(pat: &[&str], segs: &[&str]) -> bool {
    match pat.split_first() {
        None => segs.is_empty(),
        Some((&"**", rest)) => (0..=segs.len()).any(|i| match_segments(rest, &segs[i..])),
        Some((p, rest)) => segs
            .split_first()
            .is_some_and(|(s, tail)| match_segment(p.as_bytes(), s.as_bytes()) && match_segments(rest, tail)),
    }
}

fn match_segment(p: &[u8], s: &[u8]) -> bool {
    match p.split_first() {
        None => s.is_empty(),
        Some((b'*', rest)) => (0..=s.len()).any(|i| match_segment(rest, &s[i..])),
        Some((c, rest)) => s.split_first().is_some_and(|(d, tail)| c == d && match_segment(rest, tail)),
    }
}

/// How specific a pattern is: its literal characters, so `src/plugins/ui/**`
/// beats `src/plugins/**` when both match.
pub fn specificity(pattern: &str) -> usize {
    pattern.chars().filter(|&c| c != '*').count()
}

#[cfg(test)]
mod tests {
    use super::matches;

    #[test]
    fn globs() {
        assert!(matches("src/plugins/*", "src/plugins/ui"));
        assert!(!matches("src/plugins/*", "src/plugins/ui/toolbar"));
        assert!(matches("src/plugins/**", "src/plugins"));
        assert!(matches("src/plugins/**", "src/plugins/ui/toolbar"));
        assert!(matches("crates/foo-*", "crates/foo-bar"));
        assert!(matches("**/tests", "src/a/tests"));
        assert!(matches(".", "."));
        assert!(!matches("crates/*", "src/mesh"));
    }
}
