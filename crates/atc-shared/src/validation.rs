//! Deterministic, pure validation helpers shared between client and server.

/// Returns the SID that corresponds to the active runway for the same SID family,
/// or `None` if the SID is unknown.
pub fn paired_sid_for_runway(sid: &str, target_runway: &str) -> Option<&'static str> {
    match (sid, target_runway) {
        ("north1a", "36") => Some("north1b"),
        ("east1a", "36") => Some("east1b"),
        ("south1a", "36") => Some("south1b"),
        ("west1a", "36") => Some("west1b"),
        ("north1b", "18") => Some("north1a"),
        ("east1b", "18") => Some("east1a"),
        ("south1b", "18") => Some("south1a"),
        ("west1b", "18") => Some("west1a"),
        (s, r) if matches_runway(s, r) => None, // already matches, no conversion needed
        _ => None,
    }
}

fn matches_runway(sid: &str, runway: &str) -> bool {
    match (sid.ends_with('a'), sid.ends_with('b'), runway) {
        (true, _, "18") => true,
        (_, true, "36") => true,
        _ => false,
    }
}

/// Returns true if a squawk code is a plausible four-digit training code.
pub fn is_valid_squawk(code: &str) -> bool {
    code.len() == 4 && code.chars().all(|c| c.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sid_conversion_18_to_36() {
        assert_eq!(paired_sid_for_runway("east1a", "36"), Some("east1b"));
        assert_eq!(paired_sid_for_runway("north1a", "36"), Some("north1b"));
    }

    #[test]
    fn sid_conversion_36_to_18() {
        assert_eq!(paired_sid_for_runway("east1b", "18"), Some("east1a"));
    }

    #[test]
    fn sid_already_matches_runway() {
        assert_eq!(paired_sid_for_runway("east1a", "18"), None);
    }

    #[test]
    fn squawk_validation() {
        assert!(is_valid_squawk("4123"));
        assert!(!is_valid_squawk("123"));
        assert!(!is_valid_squawk("12a4"));
    }
}
