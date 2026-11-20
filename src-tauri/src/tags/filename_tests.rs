//! Every case here is a real filename out of a real library, or the exact
//! shape of one. The parser's only job is to be right about these.

use super::*;

fn parsed(stem: &str) -> (Option<String>, String) {
    let p = parse(stem);
    (p.artist, p.title)
}

#[test]
fn a_plain_artist_and_title_splits_once() {
    assert_eq!(
        parsed("Swendl - Move Move"),
        (Some("Swendl".into()), "Move Move".into())
    );
}

#[test]
fn the_official_video_suffix_and_the_video_id_both_come_off() {
    assert_eq!(
        parsed("Bob Sinclar & Kiesza - I Can't Wait (Official Video) [naCvEJABUyc]"),
        (Some("Bob Sinclar & Kiesza".into()), "I Can't Wait".into())
    );
}

#[test]
fn official_full_stream_is_noise_too() {
    assert_eq!(
        parsed("Tom & Collins & OMRI. - Dancing Shoes (Official Full Stream) [3YIsRfh7npE]"),
        (
            Some("Tom & Collins & OMRI.".into()),
            "Dancing Shoes".into()
        )
    );
}

/// The one that matters most: a remix credit is part of the record.
#[test]
fn a_remix_credit_is_kept() {
    assert_eq!(
        parsed("HAVEN. & Kaitlin Aragon - I Run (James Hype Remix) [CcIXJXOp6bc]"),
        (
            Some("HAVEN. & Kaitlin Aragon".into()),
            "I Run (James Hype Remix)".into()
        )
    );
}

#[test]
fn a_parenthetical_that_is_part_of_the_title_is_kept() {
    assert_eq!(
        parsed("Fred again.. - Delilah (pull me out of this)"),
        (
            Some("Fred again..".into()),
            "Delilah (pull me out of this)".into()
        )
    );
    assert_eq!(
        parsed("Oliver Heldens X Becky Hill - Gecko (Overdrive)"),
        (Some("Oliver Heldens X Becky Hill".into()), "Gecko (Overdrive)".into())
    );
}

/// A label in square brackets is not a video id, however much it looks like
/// the same kind of noise.
#[test]
fn a_label_in_brackets_survives_while_the_video_id_goes() {
    assert_eq!(
        parsed(
            "HUGEL, Ultra Nate, Imael Angel - Movin To The Sun \
             [Make The Girls Dance Records] [n7QlUH0zrPg]"
        ),
        (
            Some("HUGEL, Ultra Nate, Imael Angel".into()),
            "Movin To The Sun [Make The Girls Dance Records]".into()
        )
    );
}

#[test]
fn commas_in_the_artist_are_not_a_separator() {
    assert_eq!(
        parsed("Odd Mob, OMNOM, HYPERBEAM - Coming Up (It's Dare)"),
        (
            Some("Odd Mob, OMNOM, HYPERBEAM".into()),
            "Coming Up (It's Dare)".into()
        )
    );
}

/// A dash inside the title must not move the split: the first one wins.
#[test]
fn only_the_first_separator_splits() {
    assert_eq!(
        parsed("Artist - Title - Something Else"),
        (Some("Artist".into()), "Title - Something Else".into())
    );
}

#[test]
fn an_en_dash_is_a_separator_as_well() {
    assert_eq!(
        parsed("Artist \u{2013} Title"),
        (Some("Artist".into()), "Title".into())
    );
}

#[test]
fn a_name_with_no_separator_yields_a_title_and_no_artist() {
    assert_eq!(parsed("dots"), (None, "dots".into()));
    // And the noise still comes off it.
    assert_eq!(parsed("dots (Official Video)"), (None, "dots".into()));
}

#[test]
fn a_leading_track_number_is_not_an_artist() {
    assert_eq!(
        parsed("01. Artist - Title"),
        (Some("Artist".into()), "Title".into())
    );
    assert_eq!(
        parsed("07 - Artist - Title"),
        (Some("Artist".into()), "Title".into())
    );
}

/// A title that is only a number keeps itself rather than vanishing.
#[test]
fn a_numeric_name_is_left_alone() {
    assert_eq!(parsed("1979"), (None, "1979".into()));
    assert_eq!(parsed("128"), (None, "128".into()));
}

#[test]
fn the_noise_test_needs_every_word_to_be_noise() {
    assert!(is_noise("Official Video"));
    assert!(is_noise("official music video"));
    assert!(is_noise("Lyric Video"));
    assert!(is_noise("Visualiser"));
    assert!(is_noise("FREE DOWNLOAD"));
    assert!(is_noise("Audio"));
    assert!(is_noise("4K"));
    assert!(!is_noise("James Hype Remix"));
    assert!(!is_noise("Official Remix"));
    assert!(!is_noise("Extended Mix"));
    assert!(!is_noise("Live at Printworks"));
    assert!(!is_noise("Radio Edit"));
    assert!(!is_noise(""));
}

#[test]
fn a_video_id_is_eleven_url_safe_characters_that_no_word_would_spell() {
    // All from the library: two with digits, one with none at all.
    assert!(is_video_id("g8_5UQt5tgE"));
    assert!(is_video_id("6YL058XErH0"));
    assert!(is_video_id("naCvEJABUyc"));
    assert!(is_video_id("MX7h4FvFPLg"));
    // Eleven letters, but the case does not change mid-word, so it is a word.
    assert!(!is_video_id("Remastered"));
    assert!(!is_video_id("Instrumenta"));
    assert!(!is_video_id("abcdefghijk"));
    assert!(!is_video_id("SHOUTEDWORD"));
    // Wrong length, whatever it looks like.
    assert!(!is_video_id("naCvEJABUy"));
    assert!(!is_video_id("Make The Girls Dance Records"));
}

/// The case this guard exists for: a bracketed word of exactly eleven
/// characters stays in the title.
#[test]
fn an_eleven_letter_word_in_brackets_is_not_stripped() {
    assert_eq!(
        parsed("Artist - Title [Remastered]"),
        (Some("Artist".into()), "Title [Remastered]".into())
    );
}

/// Removing a group must not leave a double space or a dangling dash.
#[test]
fn removing_a_group_tidies_what_it_leaves_behind() {
    assert_eq!(clean("A (Official Video) B"), "A B");
    assert_eq!(clean("Artist - Title (Official Video) -"), "Artist - Title");
    assert_eq!(clean("Artist - Title [abcdefgh123] "), "Artist - Title");
}

/// An unclosed bracket is a name, not a parse error.
#[test]
fn an_unclosed_bracket_is_left_where_it_is() {
    assert_eq!(
        parsed("Artist - Title (Official Video"),
        (Some("Artist".into()), "Title (Official Video".into())
    );
}

/// Multi-byte characters sit either side of a removed group.
#[test]
fn accents_survive_the_clean() {
    assert_eq!(
        parsed("Vintage Culture, KVSH, Breno Miranda - Cante por Nós (Official Video)"),
        (
            Some("Vintage Culture, KVSH, Breno Miranda".into()),
            "Cante por Nós".into()
        )
    );
    assert_eq!(
        parsed("HUGEL - Movin’ To The Sun [n7QlUH0zrPg]"),
        (Some("HUGEL".into()), "Movin’ To The Sun".into())
    );
}
