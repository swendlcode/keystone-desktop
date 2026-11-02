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
    "get_tags",
    "set_tags",
    "find_replace_tags",
    "remove_extra_spaces",
    "write_key_and_tempo_tags",
    "list_tag_backups",
    "restore_tag_backup",
    "rename_files",
    "export_files",
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
            "The folders Keystone indexes.",
            json!({}),
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
            "Make a new, empty collection.",
            json!({ "name": string("What to call it.") }),
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
