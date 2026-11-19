//! What `tools/list` answers.
//!
//! The descriptions are the only documentation the model gets, so they say
//! what a tool does *to the user's library* rather than which command it sits
//! over. Two things are spelled out everywhere they apply: that a write
//! touches the actual audio files, and that `dryRun` defaults to true.

use serde_json::{json, Value};

/// Every tool name, in catalogue order. Kept beside the catalogue so a tool
/// cannot be advertised without being dispatched, or the other way round.
pub const TOOL_NAMES: &[&str] = &[
    "list_tracks",
    "get_track",
    "list_genres",
    "library_stats",
    "list_watched_folders",
    "add_watched_folder",
    "remove_watched_folder",
    "rescan_watched_folder",
    "launch_app",
    "set_key",
    "set_tempo",
    "set_energy",
    "set_rating",
    "analyze_tracks",
    "list_collections",
    "create_collection",
    "rename_collection",
    "delete_collection",
    "add_to_collection",
    "remove_from_collection",
    "get_collection_tracks",
    "reorder_collection",
    "get_tags",
    "set_tags",
    "find_replace_tags",
    "remove_extra_spaces",
    "write_key_and_tempo_tags",
    "list_tag_backups",
    "restore_tag_backup",
    "rename_files",
    "export_files",
    "export_playlist_m3u",
    "export_collection_csv",
];

fn tool(name: &str, description: &str, props: Value, required: &[&str]) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": {
            "type": "object",
            "properties": props,
            "required": required,
        },
    })
}

fn string(description: &str) -> Value {
    json!({ "type": "string", "description": description })
}

fn number(description: &str) -> Value {
    json!({ "type": "number", "description": description })
}

fn integer(description: &str) -> Value {
    json!({ "type": "integer", "description": description })
}

fn boolean(description: &str) -> Value {
    json!({ "type": "boolean", "description": description })
}

fn id_list(description: &str) -> Value {
    json!({
        "type": "array",
        "items": { "type": "string" },
        "description": description,
    })
}

const DRY_RUN: &str = "Defaults to true: report what would change without \
writing. Pass false to actually write.";

pub fn tool_list() -> Value {
    json!([
        tool(
            "list_tracks",
            "Search the library. Returns a compact row per track (id, artist, title, bpm, camelot key, energy, rating, path) plus the total matching. Use the filters rather than paging through everything.",
            json!({
                "query": string("Free text over artist, title, album, genre."),
                "camelot": string("A Camelot segment, 1A-12B, e.g. '8A'."),
                "includeCompatibleKeys": boolean("Widen `camelot` to its four harmonically compatible segments."),
                "bpmMin": number("Lowest tempo."),
                "bpmMax": number("Highest tempo."),
                "energyMin": integer("Lowest energy, 1-10."),
                "energyMax": integer("Highest energy, 1-10."),
                "ratingMin": integer("Lowest rating, 1-5."),
                "genres": json!({ "type": "array", "items": { "type": "string" }, "description": "Any of these genres." }),
                "collectionId": string("Only tracks in this collection."),
                "pathPrefix": string("Only tracks under this folder."),
                "analyzed": boolean("true for analysed tracks only, false for the backlog."),
                "needsAttention": boolean("Only tracks Keystone is unsure about: voter disagreement, low key confidence, clipping, or no analysis."),
                "sortBy": string("artist, title, album, genre, bpm, key, energy, rating, dateAdded, year, filename or path."),
                "descending": boolean("Reverse the sort."),
                "limit": integer("Rows to return, default 50, max 500."),
                "offset": integer("Rows to skip.")
            }),
            &[],
        ),
        tool(
            "get_track",
            "Everything stored about one track: the full analysis, confidences, loudness, tag values and timestamps.",
            json!({ "trackId": string("The track's id.") }),
            &["trackId"],
        ),
        tool(
            "list_genres",
            "Every genre present in the library, as the files carry it.",
            json!({}),
            &[],
        ),
        tool(
            "library_stats",
            "How many tracks there are, how many are analysed, how many are in the backlog, and how many Keystone is unsure about.",
            json!({}),
            &[],
        ),
        tool(
            "list_watched_folders",
            "The folders Keystone indexes, each with its path and whether it is active. Tracks only live under one of these, so this is where to look when the library reads as empty.",
            json!({}),
            &[],
        ),
        tool(
            "add_watched_folder",
            "Add a folder to the library and index it. Needs Keystone running: call launch_app first if it is not. Indexing runs in the background, so library_stats fills in over the following minutes rather than immediately.",
            json!({ "path": string("Absolute path to the folder.") }),
            &["path"],
        ),
        tool(
            "remove_watched_folder",
            "Stop indexing a folder. Its tracks are marked missing rather than deleted, so ratings, cues and analysis survive if it comes back. Needs Keystone running.",
            json!({ "path": string("Absolute path of a folder from list_watched_folders.") }),
            &["path"],
        ),
        tool(
            "rescan_watched_folder",
            "Walk a watched folder again and queue anything new or changed. Pass no path to rescan every active folder. Needs Keystone running.",
            json!({ "path": string("One folder to rescan. Omit for all of them.") }),
            &[],
        ),
        tool(
            "launch_app",
            "Start Keystone and wait until it answers, so the tools that need the running app will work. Returns immediately if it is already running.",
            json!({ "wait": boolean("Wait for it to come up, default true. False returns as soon as it has been started.") }),
            &[],
        ),
        tool(
            "set_key",
            "Override a track's key. Marks it as user-set, so re-analysis will not revert it.",
            json!({
                "trackId": string("The track's id."),
                "camelot": string("The correct segment, 1A-12B.")
            }),
            &["trackId", "camelot"],
        ),
        tool(
            "set_tempo",
            "Override a track's BPM. Marks it as user-set, so re-analysis will not revert it. Use this for octave errors — a drum & bass track read at 87 instead of 174.",
            json!({
                "trackId": string("The track's id."),
                "bpm": number("The correct tempo, 20-300.")
            }),
            &["trackId", "bpm"],
        ),
        tool(
            "set_energy",
            "Override a track's energy, 1-10. Marks it as user-set. Keystone's energy ordering was never validated against a human ear, so a listener's judgement beats the engine's here.",
            json!({
                "trackId": string("The track's id."),
                "energy": integer("1-10.")
            }),
            &["trackId", "energy"],
        ),
        tool(
            "set_rating",
            "Set a track's rating, 0-5. Zero clears it.",
            json!({
                "trackId": string("The track's id."),
                "rating": integer("0-5.")
            }),
            &["trackId", "rating"],
        ),
        tool(
            "analyze_tracks",
            "Queue tracks for key, tempo and energy analysis. Needs Keystone to be running, because the workers live in the app.",
            json!({
                "trackIds": id_list("Tracks to analyse."),
                "force": boolean("Re-analyse tracks that already have results.")
            }),
            &["trackIds"],
        ),
        tool(
            "list_collections",
            "The collections, with their track counts. Smart collections are rule-driven and cannot be written to.",
            json!({}),
            &[],
        ),
        tool(
            "create_collection",
            "Make a new collection, optionally with its tracks already in order.",
            json!({
                "name": string("What to call it."),
                "trackIds": id_list("Tracks to put in it, in playing order. Optional.")
            }),
            &["name"],
        ),
        tool(
            "rename_collection",
            "Rename a collection.",
            json!({
                "collectionId": string("The collection's id."),
                "name": string("The new name.")
            }),
            &["collectionId", "name"],
        ),
        tool(
            "delete_collection",
            "Delete a collection. The tracks and their files stay where they are.",
            json!({ "collectionId": string("The collection's id.") }),
            &["collectionId"],
        ),
        tool(
            "add_to_collection",
            "File tracks into a collection.",
            json!({
                "collectionId": string("The collection's id."),
                "trackIds": id_list("Tracks to add.")
            }),
            &["collectionId", "trackIds"],
        ),
        tool(
            "remove_from_collection",
            "Take tracks out of a collection. The files are untouched.",
            json!({
                "collectionId": string("The collection's id."),
                "trackIds": id_list("Tracks to remove.")
            }),
            &["collectionId", "trackIds"],
        ),
        tool(
            "get_collection_tracks",
            "A collection's tracks in playing order, each with its position. This is the one that answers what order a set runs in; list_tracks with collectionId sorts by a column instead.",
            json!({ "collectionId": string("The collection's id.") }),
            &["collectionId"],
        ),
        tool(
            "reorder_collection",
            "Set the order a collection's tracks play in. List every track in the collection, in the order you want them.",
            json!({
                "collectionId": string("The collection's id."),
                "trackIds": id_list("Every track in the collection, in the new order.")
            }),
            &["collectionId", "trackIds"],
        ),
        tool(
            "get_tags",
            "Read the tags as they stand in the file itself, not as the database cached them.",
            json!({ "trackId": string("The track's id.") }),
            &["trackId"],
        ),
        tool(
            "set_tags",
            "WRITES TO THE AUDIO FILE. Set named tag fields on one track. The replaced bytes are kept as a backup. Fields: artist, title, album, genre, grouping, comment, year, bpm, initialKey. A null or empty value clears the field.",
            json!({
                "trackId": string("The track's id."),
                "fields": json!({ "type": "object", "description": "Field name to new value." })
            }),
            &["trackId", "fields"],
        ),
        tool(
            "find_replace_tags",
            "WRITES TO AUDIO FILES. Find and replace text across tag fields, over named tracks or the whole library. Each replaced value is backed up.",
            json!({
                "find": string("Text to find. Cannot be empty."),
                "replace": string("What to put in its place. Omit to delete the text."),
                "fields": json!({ "type": "array", "items": { "type": "string" }, "description": "Which fields to search: artist, title, album, genre, grouping, comment, year, bpm, initialKey." }),
                "trackIds": id_list("Limit to these tracks. Omit for the whole library."),
                "dryRun": boolean(DRY_RUN)
            }),
            &["find", "fields"],
        ),
        tool(
            "remove_extra_spaces",
            "WRITES TO AUDIO FILES. Collapse repeated and trailing whitespace in the tag fields of the named tracks.",
            json!({
                "trackIds": id_list("Tracks to tidy."),
                "dryRun": boolean(DRY_RUN)
            }),
            &["trackIds"],
        ),
        tool(
            "write_key_and_tempo_tags",
            "WRITES TO AUDIO FILES. Write Keystone's key and energy marker into the tags, in the shape and position the user's Update Tags settings describe. Safe to re-run: a track already carrying the right marker is skipped. Refuses if the user has that setting off.",
            json!({
                "trackIds": id_list("Tracks to write. Omit for the whole library."),
                "dryRun": boolean(DRY_RUN)
            }),
            &[],
        ),
        tool(
            "list_tag_backups",
            "The tag bytes kept before each write to this track, newest first.",
            json!({ "trackId": string("The track's id.") }),
            &["trackId"],
        ),
        tool(
            "restore_tag_backup",
            "WRITES TO THE AUDIO FILE. Put a backed-up set of tags back.",
            json!({
                "trackId": string("The track's id."),
                "backupId": string("From list_tag_backups.")
            }),
            &["trackId", "backupId"],
        ),
        tool(
            "rename_files",
            "MOVES FILES ON DISK. Rename files to the pattern set in the user's settings. Always returns the plan, including why anything was skipped — a name collision, an illegal character on Windows, or no key and tempo analysed yet.",
            json!({
                "trackIds": id_list("Tracks to rename."),
                "dryRun": boolean(DRY_RUN)
            }),
            &["trackIds"],
        ),
        tool(
            "export_files",
            "Copy tracks into a folder. The originals stay where they are. Refuses to overwrite a file that is already there, and one failure does not stop the rest.",
            json!({
                "trackIds": id_list("Tracks to copy."),
                "destinationFolder": string("An existing folder to copy into.")
            }),
            &["trackIds", "destinationFolder"],
        ),
        tool(
            "export_playlist_m3u",
            "Write a collection out as an .m3u8 playlist in its stored order, with absolute paths, for loading into a DJ application.",
            json!({
                "collectionId": string("The collection's id."),
                "destinationPath": string("Where to write it. The .m3u8 extension is added if missing.")
            }),
            &["collectionId", "destinationPath"],
        ),
        tool(
            "export_collection_csv",
            "Write a collection to a CSV: number, artist, title, key, BPM, energy, genre, path. Opens correctly in Excel.",
            json!({
                "collectionId": string("The collection's id."),
                "destinationPath": string("Where to write the .csv.")
            }),
            &["collectionId", "destinationPath"],
        ),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The module note says a tool cannot be advertised without being
    /// dispatched, or the other way round. Nothing enforced it: `TOOL_NAMES`
    /// was exported and read by nothing, so the three lists could drift
    /// silently and a tool the model can see would answer "no tool named".
    #[test]
    fn the_catalogue_and_the_name_list_agree() {
        let advertised: Vec<String> = tool_list()
            .as_array()
            .expect("an array of tools")
            .iter()
            .map(|t| t["name"].as_str().expect("a name").to_string())
            .collect();
        let listed: Vec<String> = TOOL_NAMES.iter().map(|s| s.to_string()).collect();
        assert_eq!(advertised, listed, "tool_list and TOOL_NAMES disagree");
    }

    #[test]
    fn every_advertised_tool_is_dispatched() {
        let ctx = crate::mcp::McpContext::for_tests();
        for name in TOOL_NAMES {
            // `launch_app` is the one tool whose no-argument call does
            // something: it would start Keystone on whoever is running the
            // tests. It is dispatched, which is checked by the arm existing
            // in `tools::dispatch` and by the catalogue test above.
            if *name == "launch_app" {
                continue;
            }
            // Called with no arguments: almost every tool refuses, and that is
            // fine. The one answer that must never come back is the
            // unknown-tool error, which is what a missing match arm gives.
            if let Err(e) = crate::mcp::dispatch(&ctx, name, &json!({})) {
                let message = e.to_string();
                assert!(
                    !message.contains("no tool named"),
                    "{name} is advertised but not dispatched"
                );
            }
        }
    }

    /// Every entry needs a description and a coherent schema.
    ///
    /// Deliberately not a length bar: the descriptions are the model's only
    /// documentation, but `"Rename a collection."` says the whole job in four
    /// words and a minimum character count would only buy padding. What is
    /// worth enforcing is that nothing ships undocumented, and that a tool
    /// cannot require an argument it never declares — an agent sending that
    /// argument would be told it is unexpected, and omitting it would be told
    /// it is required.
    #[test]
    fn every_tool_describes_itself() {
        for entry in tool_list().as_array().expect("an array") {
            let name = entry["name"].as_str().expect("a name");
            let description = entry["description"].as_str().unwrap_or_default();
            assert!(
                !description.trim().is_empty(),
                "{name} has no description"
            );
            assert_eq!(
                entry["inputSchema"]["type"], "object",
                "{name} has no object input schema"
            );
            for required in entry["inputSchema"]["required"]
                .as_array()
                .expect("a required list")
            {
                let key = required.as_str().expect("a string");
                assert!(
                    entry["inputSchema"]["properties"].get(key).is_some(),
                    "{name} requires `{key}` but does not declare it"
                );
            }
        }
    }
}
