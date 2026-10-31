//! A collection file in an unexpected shape is reported, never guessed at.
//!
//! These are the cases that end a run before it starts: the point of each test
//! is that the message names what is wrong and says that nothing was written,
//! because the alternative is a user who cannot tell a refused export from a
//! half-applied one.

use std::path::Path;

use std::collections::HashMap;

use super::*;

const TRUNCATED: &str = include_str!("fixtures/collection-truncated.nml");
const BROKEN: &str = include_str!("fixtures/collection-broken-xml.nml");
const NOT_TRAKTOR: &str = include_str!("fixtures/not-traktor.nml");

#[test]
fn a_truncated_collection_is_reported_and_nothing_is_written() {
    let err = rewrite_document(TRUNCATED, &HashMap::new(), false).unwrap_err();
    assert!(err.to_string().contains("truncated"), "{err}");
}

#[test]
fn broken_xml_is_reported_rather_than_half_parsed() {
    let err = rewrite_document(BROKEN, &HashMap::new(), false).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("not valid XML") || msg.contains("truncated"),
        "{msg}"
    );
    assert!(msg.contains("Nothing was written") || msg.contains("will not write"), "{msg}");
}

#[test]
fn another_apps_library_file_is_recognised_as_not_a_collection() {
    let err = rewrite_document(NOT_TRAKTOR, &HashMap::new(), false).unwrap_err();
    assert!(err.to_string().contains("no NML root element"), "{err}");
}

#[test]
fn a_collection_with_no_collection_element_is_refused() {
    let headless = "<NML VERSION=\"19\"><HEAD></HEAD></NML>";
    let err = rewrite_document(headless, &HashMap::new(), false).unwrap_err();
    assert!(err.to_string().contains("no COLLECTION element"), "{err}");
}

#[test]
fn a_missing_collection_file_is_a_clear_refusal_not_an_io_error() {
    let err = read_collection(Path::new("/keystone-no-such-dir/collection.nml")).unwrap_err();
    assert!(err.to_string().contains("Point Keystone at collection.nml"), "{err}");
}
