//! The name a cue carries into a DJ app.
//!
//! Shared by both exporters so a cue reads the same on a CDJ screen as on a
//! Traktor deck. The user's own label wins; otherwise the role S11 assigned,
//! which is worth more to a DJ than `CUE 3`.

use crate::models::CuePoint;

pub fn cue_label(cue: &CuePoint) -> String {
    if let Some(label) = cue.label.as_deref().filter(|l| !l.trim().is_empty()) {
        return label.to_string();
    }
    title_case(cue.kind.as_str())
}

/// `first_beat` -> `First Beat`.
fn title_case(slug: &str) -> String {
    slug.split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::export::testkit;
    use crate::models::CueKind;

    #[test]
    fn the_role_becomes_a_readable_name() {
        assert_eq!(cue_label(&testkit::cue(0, 0.0, CueKind::FirstBeat)), "First Beat");
        assert_eq!(cue_label(&testkit::cue(0, 0.0, CueKind::Drop)), "Drop");
    }

    #[test]
    fn a_users_own_label_wins_and_blank_does_not() {
        let mut cue = testkit::cue(0, 0.0, CueKind::Drop);
        cue.label = Some("  ".into());
        assert_eq!(cue_label(&cue), "Drop");
        cue.label = Some("Second drop".into());
        assert_eq!(cue_label(&cue), "Second drop");
    }
}
