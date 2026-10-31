//! Surgical attribute edits on a raw tag.
//!
//! Setting one attribute by rewriting its value in place, rather than by
//! rebuilding the element, is the only way to change `INFO/@KEY` or
//! `TEMPO/@BPM` without touching the dozen attributes beside them that
//! Keystone does not model. Reserialising would reorder them, requote them and
//! drop the ones we never read.
//!
//! The scanner walks the tag properly — name, `=`, quote, value — instead of
//! searching for `NAME="`. A collection whose comment field happens to contain
//! `KEY=` would otherwise have that text edited instead of the attribute.

use std::ops::Range;

/// The value of `name`, exactly as it appears in the source (still escaped).
pub fn get(tag: &str, name: &str) -> Option<String> {
    find(tag, name).map(|r| tag[r].to_string())
}

/// Replaces `name`'s value, or adds the attribute when it is absent.
pub fn set(tag: &str, name: &str, value: &str) -> String {
    let escaped = escape(value);
    match find(tag, name) {
        Some(range) => {
            let mut out = String::with_capacity(tag.len() + escaped.len());
            out.push_str(&tag[..range.start]);
            out.push_str(&escaped);
            out.push_str(&tag[range.end..]);
            out
        }
        None => {
            // Inserted before the terminator so that `<X/>` stays empty and
            // `<X>` stays a start tag.
            let insert_at = terminator(tag);
            let mut out = String::with_capacity(tag.len() + name.len() + escaped.len() + 4);
            out.push_str(tag[..insert_at].trim_end());
            out.push_str(&format!(" {name}=\"{escaped}\""));
            out.push_str(&tag[insert_at..]);
            out
        }
    }
}

/// Where the tag stops carrying attributes: before `/>` or before `>`.
fn terminator(tag: &str) -> usize {
    if let Some(stripped) = tag.strip_suffix("/>") {
        return stripped.len();
    }
    if let Some(stripped) = tag.strip_suffix('>') {
        return stripped.len();
    }
    tag.len()
}

pub fn escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

/// The byte range of `name`'s value inside `tag`, or `None` when the attribute
/// is absent. Stops at the first thing it cannot parse rather than guessing:
/// an attribute we failed to find is added, and adding a duplicate to a tag we
/// misread would make the document invalid.
fn find(tag: &str, name: &str) -> Option<Range<usize>> {
    let b = tag.as_bytes();
    let mut i = 0;
    // Past `<` and the element name.
    while i < b.len() && !b[i].is_ascii_whitespace() {
        i += 1;
    }
    loop {
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= b.len() || b[i] == b'>' || b[i] == b'/' {
            return None;
        }
        let name_start = i;
        while i < b.len() && !b[i].is_ascii_whitespace() && b[i] != b'=' {
            i += 1;
        }
        let attr_name = &tag[name_start..i];
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= b.len() || b[i] != b'=' {
            return None;
        }
        i += 1;
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= b.len() || (b[i] != b'"' && b[i] != b'\'') {
            return None;
        }
        let quote = b[i];
        i += 1;
        let value_start = i;
        while i < b.len() && b[i] != quote {
            i += 1;
        }
        if i >= b.len() {
            return None;
        }
        let value_end = i;
        i += 1;
        if attr_name == name {
            return Some(value_start..value_end);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const INFO: &str = r#"<INFO BITRATE="320000" GENRE="House" KEY="Am" PLAYCOUNT="3" RANKING="153">"#;

    #[test]
    fn setting_an_attribute_leaves_every_other_byte_alone() {
        let out = set(INFO, "KEY", "8A");
        assert_eq!(
            out,
            r#"<INFO BITRATE="320000" GENRE="House" KEY="8A" PLAYCOUNT="3" RANKING="153">"#
        );
    }

    #[test]
    fn an_absent_attribute_is_added_inside_the_tag() {
        let out = set(r#"<INFO BITRATE="320000">"#, "KEY", "8A");
        assert_eq!(out, r#"<INFO BITRATE="320000" KEY="8A">"#);
        let empty = set(r#"<MUSICAL_KEY/>"#, "VALUE", "9");
        assert_eq!(empty, r#"<MUSICAL_KEY VALUE="9"/>"#);
        // And a self-closing tag stays self-closing even with a space before it.
        let spaced = set(r#"<MUSICAL_KEY />"#, "VALUE", "9");
        assert_eq!(spaced, r#"<MUSICAL_KEY VALUE="9"/>"#);
    }

    #[test]
    fn a_value_that_merely_mentions_the_attribute_name_is_not_edited() {
        // The failure this scanner exists to prevent: a comment containing
        // `KEY=` being rewritten instead of the KEY attribute.
        let tag = r#"<INFO COMMENT="tagged KEY=Am by hand" KEY="Am">"#;
        let out = set(tag, "KEY", "8A");
        assert_eq!(out, r#"<INFO COMMENT="tagged KEY=Am by hand" KEY="8A">"#);
    }

    #[test]
    fn single_quoted_values_are_found_too() {
        let tag = "<INFO KEY='Am' GENRE='House'>";
        assert_eq!(get(tag, "KEY").as_deref(), Some("Am"));
        assert_eq!(set(tag, "KEY", "8A"), "<INFO KEY='8A' GENRE='House'>");
    }

    #[test]
    fn a_name_that_is_a_prefix_of_another_attribute_is_not_confused_for_it() {
        let tag = r#"<CUE_V2 START="100.0" START_OFFSET="5">"#;
        assert_eq!(get(tag, "START").as_deref(), Some("100.0"));
        let tag = r#"<CUE_V2 START_OFFSET="5" START="100.0">"#;
        assert_eq!(get(tag, "START").as_deref(), Some("100.0"));
    }

    #[test]
    fn written_values_are_escaped() {
        let out = set(r#"<INFO KEY="Am">"#, "KEY", r#"A & "B""#);
        assert_eq!(out, r#"<INFO KEY="A &amp; &quot;B&quot;">"#);
        // Which stays parseable, unlike the raw value.
        assert!(quick_xml::Reader::from_str(&format!("{out}</INFO>"))
            .read_event()
            .is_ok());
    }

    #[test]
    fn a_missing_attribute_reads_as_none_rather_than_an_empty_string() {
        assert!(get(r#"<INFO BITRATE="320000">"#, "KEY").is_none());
        assert_eq!(get(INFO, "KEY").as_deref(), Some("Am"));
    }
}
