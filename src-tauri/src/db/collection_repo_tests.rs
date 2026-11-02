use crate::db::test_support::Harness;
use crate::models::{CollectionKind, IndexStatus, TrackFilters};

fn user(h: &Harness, name: &str) -> String {
    h.collections
        .create(name, CollectionKind::User, None)
        .unwrap()
        .id
}

#[test]
fn a_new_collection_is_appended_to_the_order_the_user_already_set() {
    let h = Harness::new();
    let first = user(&h, "Swendl Crate - Miami");
    let second = user(&h, "Selected");

    let list = h.collections.list().unwrap();
    assert_eq!(
        list.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(),
        ["Swendl Crate - Miami", "Selected"]
    );
    assert_eq!(list[0].position, 0);
    assert_eq!(list[1].position, 1);
    assert_eq!(list[0].id, first);
    assert_eq!(list[1].id, second);
    assert_eq!(list[0].kind, CollectionKind::User);
}

#[test]
fn rename_colour_and_delete_do_what_the_sidebar_offers() {
    let h = Harness::new();
    let id = user(&h, "My Collection");
    h.collections.rename(&id, "Miami 2026").unwrap();
    h.collections.set_color(&id, Some("#99F23F")).unwrap();

    let c = h.collections.get(&id).unwrap().unwrap();
    assert_eq!(c.name, "Miami 2026");
    assert_eq!(c.color.as_deref(), Some("#99F23F"));

    h.collections.set_color(&id, None).unwrap();
    assert!(h.collections.get(&id).unwrap().unwrap().color.is_none());

    h.collections.delete(&id).unwrap();
    assert!(h.collections.get(&id).unwrap().is_none());
    assert!(h.collections.list().unwrap().is_empty());
}

#[test]
fn reordering_takes_the_whole_list_so_the_sidebar_cannot_show_half_an_order() {
    let h = Harness::new();
    let a = user(&h, "A");
    let b = user(&h, "B");
    let c = user(&h, "C");

    h.collections
        .reorder(&[c.clone(), a.clone(), b.clone()])
        .unwrap();
    let names: Vec<String> = h
        .collections
        .list()
        .unwrap()
        .into_iter()
        .map(|c| c.name)
        .collect();
    assert_eq!(names, ["C", "A", "B"]);
}

#[test]
fn adding_the_same_track_twice_is_a_no_op_and_leaves_no_gap_in_the_order() {
    let h = Harness::new();
    let id = user(&h, "Crate");
    let one = h.add("/m/one.mp3");
    let two = h.add("/m/two.mp3");

    assert_eq!(
        h.collections.add_tracks(&id, &[one.clone(), two.clone()]).unwrap(),
        2
    );
    // A second drop of the same track must not duplicate it, because the drop
    // target is one drag away from exactly this.
    assert_eq!(h.collections.add_tracks(&id, &[one.clone()]).unwrap(), 0);
    assert_eq!(h.collections.track_ids(&id).unwrap(), [one.clone(), two.clone()]);

    let three = h.add("/m/three.mp3");
    h.collections.add_tracks(&id, &[three.clone()]).unwrap();
    assert_eq!(h.collections.track_ids(&id).unwrap(), [one, two, three]);
}

#[test]
fn a_crate_keeps_the_order_the_dj_dragged_it_into() {
    let h = Harness::new();
    let id = user(&h, "Set");
    let ids: Vec<String> = (0..4).map(|i| h.add(&format!("/m/{i}.mp3"))).collect();
    h.collections.add_tracks(&id, &ids).unwrap();

    let reversed: Vec<String> = ids.iter().rev().cloned().collect();
    h.collections.set_track_positions(&id, &reversed).unwrap();
    assert_eq!(h.collections.track_ids(&id).unwrap(), reversed);
}

#[test]
fn removing_a_track_from_a_crate_leaves_the_track_in_the_library() {
    let h = Harness::new();
    let id = user(&h, "Crate");
    let one = h.add("/m/one.mp3");
    let two = h.add("/m/two.mp3");
    h.collections.add_tracks(&id, &[one.clone(), two.clone()]).unwrap();

    assert_eq!(h.collections.remove_tracks(&id, &[one.clone()]).unwrap(), 1);
    assert_eq!(h.collections.track_ids(&id).unwrap(), [two]);
    assert!(h.tracks.get_by_id(&one).unwrap().is_some());
}

#[test]
fn deleting_a_track_removes_it_from_every_crate_it_was_in() {
    let h = Harness::new();
    let a = user(&h, "A");
    let b = user(&h, "B");
    let track = h.add("/m/one.mp3");
    h.collections.add_tracks(&a, &[track.clone()]).unwrap();
    h.collections.add_tracks(&b, &[track.clone()]).unwrap();

    h.tracks.delete(&track).unwrap();
    assert!(h.collections.track_ids(&a).unwrap().is_empty());
    assert!(h.collections.track_ids(&b).unwrap().is_empty());
}

#[test]
fn deleting_a_crate_leaves_every_track_in_the_library() {
    let h = Harness::new();
    let id = user(&h, "Crate");
    let track = h.add("/m/one.mp3");
    h.collections.add_tracks(&id, &[track.clone()]).unwrap();

    h.collections.delete(&id).unwrap();
    assert!(h.tracks.get_by_id(&track).unwrap().is_some());
}

#[test]
fn a_track_id_that_is_no_longer_in_the_library_is_skipped_not_fatal() {
    let h = Harness::new();
    let id = user(&h, "Crate");
    let one = h.add("/m/one.mp3");
    let two = h.add("/m/two.mp3");

    // The shape an agent produces: a list fetched a minute ago, one of whose
    // rows has since been removed. `INSERT OR IGNORE` does not cover this —
    // SQLite's conflict clause does not apply to FOREIGN KEY constraints — so
    // before the `WHERE EXISTS` the unknown id aborted the statement and the
    // transaction took the two good tracks down with it.
    let added = h
        .collections
        .add_tracks(&id, &[one.clone(), "no-such-track".to_string(), two.clone()])
        .expect("an unknown id must not fail the batch");

    assert_eq!(added, 2, "both real tracks are filed");
    assert_eq!(
        h.collections.track_ids(&id).unwrap(),
        vec![one, two],
        "and in order, with no gap left by the one that was skipped"
    );
}

#[test]
fn the_count_beside_the_name_is_the_number_the_table_will_show() {
    let h = Harness::new();
    let id = user(&h, "Crate");
    let present = h.add("/m/present.mp3");
    let gone = h.add("/m/gone.mp3");
    h.collections
        .add_tracks(&id, &[present, gone.clone()])
        .unwrap();
    assert_eq!(h.collections.list().unwrap()[0].track_count, 2);

    // An unmounted drive must not inflate the number beside the crate's name.
    h.tracks
        .set_index_status(&gone, IndexStatus::Missing, None)
        .unwrap();
    assert_eq!(h.collections.list().unwrap()[0].track_count, 1);
}

#[test]
fn a_smart_collection_counts_by_running_its_rule() {
    let h = Harness::new();
    let unsure = h.add("/m/unsure.mp3");
    h.set_analysis_columns(&unsure, "analysis_version = 1, key_confidence = 0.3");
    let fine = h.add("/m/fine.mp3");
    h.set_analysis_columns(&fine, "analysis_version = 1, key_confidence = 0.97");

    let rule = serde_json::to_string(&TrackFilters {
        improvable_only: true,
        ..TrackFilters::default()
    })
    .unwrap();
    let smart = h
        .collections
        .create("Improve Tracks", CollectionKind::Smart, Some(&rule))
        .unwrap();
    assert_eq!(smart.kind, CollectionKind::Smart);
    assert_eq!(smart.track_count, 1);
    // No membership rows: the rule *is* the membership.
    assert!(h.collections.track_ids(&smart.id).unwrap().is_empty());

    // The count follows the library rather than a snapshot.
    let another = h.add("/m/also-unsure.mp3");
    h.set_analysis_columns(&another, "analysis_version = 1, key_confidence = 0.2");
    assert_eq!(h.collections.get(&smart.id).unwrap().unwrap().track_count, 2);
}

#[test]
fn a_smart_rule_that_no_longer_parses_counts_zero_rather_than_breaking_the_sidebar() {
    let h = Harness::new();
    h.add("/m/one.mp3");
    let smart = h
        .collections
        .create("Broken", CollectionKind::Smart, Some("{not json"))
        .unwrap();
    assert_eq!(smart.track_count, 0);
    assert_eq!(h.collections.list().unwrap().len(), 1);
}

#[test]
fn a_collection_scopes_the_library_query() {
    let h = Harness::new();
    let id = user(&h, "Crate");
    let inside = h.add("/m/inside.mp3");
    h.add("/m/outside.mp3");
    h.collections.add_tracks(&id, &[inside.clone()]).unwrap();

    let n = h
        .tracks
        .count(&TrackFilters {
            collection_id: Some(id),
            ..TrackFilters::default()
        })
        .unwrap();
    assert_eq!(n, 1);
}

/// Two writers filing tracks at the same time must both succeed.
///
/// `add_tracks` reads `MAX(position)` and then inserts. Under
/// `Connection::transaction()` — DEFERRED — that takes a read lock and then
/// asks SQLite to upgrade it, and SQLite refuses the upgrade outright with
/// `SQLITE_BUSY` when another connection has committed in between. The busy
/// handler is deliberately not consulted for an upgrade, so `busy_timeout`
/// does not cover it and no amount of waiting helps: six concurrent writers
/// lost 7 of ~180 calls to `database is locked`.
///
/// This is not an MCP-only path. Filing tracks into a crate while the indexer
/// writes is the same two connections doing the same thing.
#[test]
fn two_writers_filing_at_once_do_not_collide() {
    use std::sync::Arc;
    use std::thread;

    let h = Harness::new();
    let tracks: Vec<String> = (0..12).map(|i| h.add(&format!("/m/t{i}.mp3"))).collect();
    let crates: Vec<String> = (0..4).map(|i| user(&h, &format!("Crate {i}"))).collect();

    let collections = Arc::new(h.collections.clone());
    let mut handles = Vec::new();
    for (w, crate_id) in crates.iter().cloned().enumerate() {
        let collections = collections.clone();
        let tracks = tracks.clone();
        handles.push(thread::spawn(move || {
            let mut errors = Vec::new();
            for round in 0..6 {
                // Overlapping slices, so the writers contend rather than
                // politely taking turns on separate rows.
                let slice: Vec<String> = tracks
                    .iter()
                    .skip((w + round) % 4)
                    .take(6)
                    .cloned()
                    .collect();
                if let Err(e) = collections.add_tracks(&crate_id, &slice) {
                    errors.push(e.to_string());
                }
                if let Err(e) = collections.remove_tracks(&crate_id, &slice) {
                    errors.push(e.to_string());
                }
            }
            errors
        }));
    }

    let errors: Vec<String> = handles
        .into_iter()
        .flat_map(|h| h.join().expect("worker panicked"))
        .collect();

    assert!(
        errors.is_empty(),
        "concurrent writers must not fail; got {} errors, first: {:?}",
        errors.len(),
        errors.first()
    );
}
