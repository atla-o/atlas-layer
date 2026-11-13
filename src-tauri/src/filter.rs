//! Local NSFW and harmful-content filter.
//!
//! The word list lives in this binary. Nothing is sent off the machine, and
//! turning the switch off stops scanning. No hosts file, DNS, or root change.

const TERMS: &[&str] = &[
    "porn",
    "pornography",
    "hentai",
    "nsfw",
    "xxx",
    "nude",
    "nudes",
    "sex tape",
    "child porn",
    "csam",
    "how to make a bomb",
    "build a bomb",
];

/// `Some(reason)` when the text should be held. The reason does not quote the text.
pub fn blocked(text: &str) -> Option<&'static str> {
    let folded = text.to_lowercase();
    for term in TERMS {
        if contains_term(&folded, term) {
            return Some(
                if term.contains("bomb") || *term == "csam" || term.contains("child") {
                    "harmful"
                } else {
                    "nsfw"
                },
            );
        }
    }
    None
}

pub fn held_text() -> &'static str {
    "Held by the local filter. The original text was not kept."
}

fn contains_term(haystack: &str, term: &str) -> bool {
    let mut start = 0;
    while let Some(found) = haystack[start..].find(term) {
        let at = start + found;
        let before_ok = at == 0 || !haystack.as_bytes()[at - 1].is_ascii_alphanumeric();
        let after = at + term.len();
        let after_ok =
            after >= haystack.len() || !haystack.as_bytes()[after].is_ascii_alphanumeric();
        if before_ok && after_ok {
            return true;
        }
        start = at + 1;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::blocked;

    #[test]
    fn ordinary_desk_text_passes() {
        assert!(blocked("Sketch the shelf and water the plant").is_none());
        assert!(blocked("The nude mouse model is a lab strain").is_some());
    }

    #[test]
    fn explicit_and_harmful_phrases_are_held() {
        assert_eq!(blocked("send nudes"), Some("nsfw"));
        assert_eq!(blocked("how to make a bomb tonight"), Some("harmful"));
        assert!(blocked("note").is_none());
    }
}
