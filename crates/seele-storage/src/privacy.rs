//! Privacy stripping — removes `<private>...</private>` blocks before
//! content is hashed for dedup or indexed by FTS5.
//!
//! Inherited from ENGRAM. Multiline + case-insensitive. Malformed blocks
//! (no closing tag) are left untouched on purpose — silent silent stripping
//! would surprise the caller.

use once_cell::sync::Lazy;
use regex::Regex;

static PRIVATE_TAG_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?si)<private>.*?</private>").expect("private tag regex must compile")
});

/// Remove every `<private>...</private>` block from `s`. Blocks may span
/// multiple lines; matching is case-insensitive on the tags. Unclosed
/// `<private>` tags are left as-is.
pub fn strip_private_tags(s: &str) -> String {
    PRIVATE_TAG_RE.replace_all(s, "").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_single_line_block() {
        let s = "Public <private>secret</private> rest.";
        assert_eq!(strip_private_tags(s), "Public  rest.");
    }

    #[test]
    fn strips_multiline_block() {
        let s = "Hi\n<private>\nlinea 1\nlinea 2\n</private>\nbye";
        let out = strip_private_tags(s);
        assert!(!out.contains("linea 1"));
        assert!(!out.contains("linea 2"));
        assert!(out.contains("Hi"));
        assert!(out.contains("bye"));
    }

    #[test]
    fn no_tags_unchanged() {
        let s = "no secrets here";
        assert_eq!(strip_private_tags(s), s);
    }

    #[test]
    fn unclosed_tag_left_intact() {
        // Malformed input — caller's bug, we don't paper over it.
        let s = "<private>oops never closed";
        assert_eq!(strip_private_tags(s), s);
    }

    #[test]
    fn case_insensitive_tag_names() {
        let s = "before <PRIVATE>x</PRIVATE> after";
        assert_eq!(strip_private_tags(s), "before  after");
    }

    #[test]
    fn multiple_blocks_all_stripped() {
        let s = "a<private>1</private>b<private>2</private>c";
        assert_eq!(strip_private_tags(s), "abc");
    }

    #[test]
    fn nested_inner_tag_treated_as_text_until_first_close() {
        // Greedy/lazy choice: lazy regex stops at first </private>.
        let s = "<private>outer<private>inner</private>tail</private>";
        let out = strip_private_tags(s);
        // First </private> closes the first <private>; remaining text stays.
        assert_eq!(out, "tail</private>");
    }
}
