//! Reading playlists out of the Apple Music / iTunes library XML, and out of
//! ordinary `.m3u` / `.m3u8` files.
//!
//! Importing a playlist is a one-way read of names and file paths. Nothing is
//! written back, no library is opened and no API is called: Apple's XML is a
//! file on disk that the user has already chosen to publish, and `.m3u` is a
//! list of paths. The playlist itself is not recreated as a Keystone
//! collection either — this hands the indexer a set of files, which is what
//! "add these to my library" means. Filing them is a separate gesture the user
//! makes afterwards, against collections they already have.
//!
//! The XML is parsed with `quick_xml` rather than a plist crate. The file is a
//! plist, but only four of its keys matter — `Tracks`, `Playlists`, `Name`,
//! `Location` — and a general plist reader would build a tree of every tag,
//! comment, rating and play count in a library that runs to tens of megabytes.
//! The readers below skip any element they do not need without allocating it.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use quick_xml::escape::unescape;
use quick_xml::events::Event;
use quick_xml::Reader;
use serde::Serialize;

use crate::core::scanner;
use crate::error::{KeystoneError, Result};

/// One importable playlist: what it is called and how big it is.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MusicPlaylist {
    pub name: String,
    /// Entries that resolve to a local audio file that exists right now.
    ///
    /// Not the playlist's length. An Apple Music subscription playlist is
    /// mostly streaming entries with no `Location` at all, and a count of 173
    /// next to an import that adds four is worse than no count.
    pub track_count: usize,
}

/// What the Music import dialog needs to draw itself.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MusicLibraryInfo {
    /// Where the XML was read from, or null when there is none to read.
    pub xml_path: Option<String>,
    pub playlists: Vec<MusicPlaylist>,
}

/// The library XML, if the user's Music app is publishing one.
///
/// Catalina moved the library to a binary `.musicdb` and made the XML opt-in
/// behind Settings -> Advanced -> "Share Library XML with other
/// applications". Both locations are checked because an upgraded Mac keeps the
/// old iTunes file around, and the newest one wins.
pub fn library_xml_path() -> Option<PathBuf> {
    let home = PathBuf::from(std::env::var_os("HOME")?);
    [
        "Music/Music/Music Library.xml",
        "Music/iTunes/iTunes Music Library.xml",
        "Music/iTunes/iTunes Library.xml",
    ]
    .iter()
    .map(|rel| home.join(rel))
    .find(|p| p.is_file())
}

/// Every named playlist in the XML, with the count of files it can actually
/// contribute. Folders and Apple's own built-in lists are left out.
pub fn read_playlists(xml: &Path) -> Result<Vec<MusicPlaylist>> {
    let parsed = parse(&std::fs::read_to_string(xml)?)?;
    Ok(parsed
        .playlists
        .iter()
        .map(|p| MusicPlaylist {
            name: p.name.clone(),
            track_count: resolve(&parsed.locations, &p.track_ids).len(),
        })
        .collect())
}

/// The existing local audio files behind a set of playlist names.
///
/// Deduplicated across playlists, because a track in both Chill and Dance is
/// one file and queueing it twice is two analyses of the same bytes.
pub fn playlist_files(xml: &Path, names: &[String]) -> Result<Vec<PathBuf>> {
    let parsed = parse(&std::fs::read_to_string(xml)?)?;
    let mut out: Vec<PathBuf> = Vec::new();
    let mut seen: Vec<PathBuf> = Vec::new();

    for playlist in parsed.playlists.iter().filter(|p| names.contains(&p.name)) {
        for path in resolve(&parsed.locations, &playlist.track_ids) {
            if !seen.contains(&path) {
                seen.push(path.clone());
                out.push(path);
            }
        }
    }
    Ok(out)
}

/// An `.m3u` / `.m3u8` / `.pls` playlist as a list of existing audio files.
///
/// Every DJ tool exports one of these, which makes it the import path that
/// does not depend on Apple publishing anything. Relative entries resolve
/// against the playlist's own folder, which is what the format says and what
/// makes an exported folder portable.
pub fn read_playlist_file(path: &Path) -> Result<Vec<PathBuf>> {
    let text = read_text_lossy(path)?;
    let base = path.parent().unwrap_or(Path::new("."));
    let mut out = Vec::new();

    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // `.pls` entries are `FileN=/path/to.mp3`; anything before the first
        // `=` on such a line is the key, not part of the path.
        let entry = match line.split_once('=') {
            Some((key, value)) if key.trim_start().to_lowercase().starts_with("file") => value,
            _ => line,
        };

        let candidate = if entry.starts_with("file://") {
            match location_to_path(entry) {
                Some(p) => p,
                None => continue,
            }
        } else {
            let p = PathBuf::from(entry);
            if p.is_absolute() {
                p
            } else {
                base.join(p)
            }
        };

        if usable(&candidate) && !out.contains(&candidate) {
            out.push(candidate);
        }
    }
    Ok(out)
}

/// A playlist's track ids as paths that exist and that Keystone can decode.
fn resolve(locations: &HashMap<i64, String>, ids: &[i64]) -> Vec<PathBuf> {
    ids.iter()
        .filter_map(|id| locations.get(id))
        .filter_map(|loc| location_to_path(loc))
        .filter(|p| usable(p))
        .collect()
}

fn usable(path: &Path) -> bool {
    scanner::audio_extension(path).is_some() && path.is_file()
}

/// Reads a file that should be UTF-8 and may not be.
///
/// A hand-edited `.m3u` is routinely Latin-1, and refusing the whole import
/// over one mangled accent in one filename loses the other four hundred
/// entries. The bad line fails its own existence check instead.
fn read_text_lossy(path: &Path) -> Result<String> {
    Ok(String::from_utf8_lossy(&std::fs::read(path)?).into_owned())
}

/// `file:///Users/me/Music/a%20track.mp3` -> `/Users/me/Music/a track.mp3`.
///
/// Returns None for anything that is not a local absolute path, which is how
/// network shares and Apple Music's streaming entries drop out.
fn location_to_path(loc: &str) -> Option<PathBuf> {
    let rest = loc
        .strip_prefix("file://localhost")
        .or_else(|| loc.strip_prefix("file://"))?;
    let decoded = percent_decode(rest);
    decoded.starts_with('/').then(|| PathBuf::from(decoded))
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(hi), Some(lo)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                out.push(hi * 16 + lo);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

// ── The plist reader ────────────────────────────────────────────────────────

struct Parsed {
    /// Track ID -> `Location` URL, for the tracks that have one.
    locations: HashMap<i64, String>,
    playlists: Vec<RawPlaylist>,
}

struct RawPlaylist {
    name: String,
    track_ids: Vec<i64>,
}

/// A child element of the container being read.
enum Child {
    /// `<dict>`, `<array>`, `<string>` … — its content is still unread.
    Open(String),
    /// `<true/>`, `<array/>` — nothing follows.
    Empty(String),
    /// The container closed, or the document ended.
    Closed,
}

fn parse(xml: &str) -> Result<Parsed> {
    let mut rd = Reader::from_str(xml);

    descend(&mut rd, "plist")?;
    descend(&mut rd, "dict")?;

    let mut locations = HashMap::new();
    let mut playlists = Vec::new();

    while let Some((key, child)) = next_pair(&mut rd)? {
        match child {
            Child::Empty(_) => {}
            Child::Open(tag) => match (key.as_str(), tag.as_str()) {
                ("Tracks", "dict") => locations = read_tracks(&mut rd)?,
                ("Playlists", "array") => playlists = read_playlists_array(&mut rd)?,
                _ => skip_element(&mut rd)?,
            },
            Child::Closed => break,
        }
    }

    Ok(Parsed {
        locations,
        playlists,
    })
}

/// Walks forward to the named element and steps inside it.
fn descend(rd: &mut Reader<&[u8]>, want: &str) -> Result<()> {
    loop {
        match next_child(rd)? {
            Child::Open(tag) if tag == want => return Ok(()),
            Child::Open(_) => skip_element(rd)?,
            Child::Empty(_) => {}
            Child::Closed => {
                return Err(KeystoneError::Other(format!(
                    "the Music library XML has no <{want}> — it may be truncated"
                )))
            }
        }
    }
}

/// The Tracks dict: every entry is `<key>12345</key><dict>…</dict>`.
fn read_tracks(rd: &mut Reader<&[u8]>) -> Result<HashMap<i64, String>> {
    let mut out = HashMap::new();
    while let Some((_, child)) = next_pair(rd)? {
        match child {
            Child::Open(tag) if tag == "dict" => {
                if let (Some(id), Some(loc)) = read_track(rd)? {
                    out.insert(id, loc);
                }
            }
            Child::Open(_) => skip_element(rd)?,
            Child::Empty(_) | Child::Closed => {}
        }
    }
    Ok(out)
}

fn read_track(rd: &mut Reader<&[u8]>) -> Result<(Option<i64>, Option<String>)> {
    let mut id = None;
    let mut location = None;
    while let Some((key, child)) = next_pair(rd)? {
        match child {
            Child::Open(tag) => match (key.as_str(), tag.as_str()) {
                ("Track ID", "integer") => id = read_text(rd)?.parse().ok(),
                ("Location", "string") => location = Some(read_text(rd)?),
                _ => skip_element(rd)?,
            },
            Child::Empty(_) | Child::Closed => {}
        }
    }
    Ok((id, location))
}

fn read_playlists_array(rd: &mut Reader<&[u8]>) -> Result<Vec<RawPlaylist>> {
    let mut out = Vec::new();
    loop {
        match next_child(rd)? {
            Child::Closed => break,
            Child::Open(tag) if tag == "dict" => {
                if let Some(p) = read_playlist(rd)? {
                    out.push(p);
                }
            }
            Child::Open(_) => skip_element(rd)?,
            Child::Empty(_) => {}
        }
    }
    Ok(out)
}

/// One playlist, or None when it is not something a user can import.
///
/// Apple's own lists carry a `Distinguished Kind` — Library, Music, Movies,
/// Downloaded — and a folder is a container with no items of its own. Offering
/// either is offering a row that cannot do anything.
fn read_playlist(rd: &mut Reader<&[u8]>) -> Result<Option<RawPlaylist>> {
    let mut name = String::new();
    let mut skip = false;
    let mut track_ids = Vec::new();

    while let Some((key, child)) = next_pair(rd)? {
        match child {
            Child::Empty(tag) => {
                if tag == "true" && (key == "Master" || key == "Folder" || key == "Visible") {
                    skip = key != "Visible";
                }
            }
            Child::Open(tag) => match (key.as_str(), tag.as_str()) {
                ("Name", "string") => name = read_text(rd)?,
                ("Playlist Items", "array") => track_ids = read_playlist_items(rd)?,
                ("Distinguished Kind", _) => {
                    skip = true;
                    skip_element(rd)?;
                }
                _ => skip_element(rd)?,
            },
            Child::Closed => break,
        }
    }

    if skip || name.is_empty() || track_ids.is_empty() {
        return Ok(None);
    }
    Ok(Some(RawPlaylist { name, track_ids }))
}

fn read_playlist_items(rd: &mut Reader<&[u8]>) -> Result<Vec<i64>> {
    let mut ids = Vec::new();
    loop {
        match next_child(rd)? {
            Child::Closed => break,
            Child::Open(tag) if tag == "dict" => {
                while let Some((key, child)) = next_pair(rd)? {
                    match child {
                        Child::Open(t) if key == "Track ID" && t == "integer" => {
                            if let Ok(id) = read_text(rd)?.parse::<i64>() {
                                ids.push(id);
                            }
                        }
                        Child::Open(_) => skip_element(rd)?,
                        Child::Empty(_) | Child::Closed => {}
                    }
                }
            }
            Child::Open(_) => skip_element(rd)?,
            Child::Empty(_) => {}
        }
    }
    Ok(ids)
}

/// The next `<key>k</key><value>` pair, or None when the dict closed.
fn next_pair(rd: &mut Reader<&[u8]>) -> Result<Option<(String, Child)>> {
    let key = match next_child(rd)? {
        Child::Closed => return Ok(None),
        Child::Open(tag) if tag == "key" => read_text(rd)?,
        // A dict whose child is not a key is not a plist dict. Skipping the
        // stray element is kinder than failing the import: the rest of the
        // file is still readable and the user still gets their playlists.
        Child::Open(_) => {
            skip_element(rd)?;
            return Ok(Some((String::new(), Child::Empty(String::new()))));
        }
        Child::Empty(_) => return Ok(Some((String::new(), Child::Empty(String::new())))),
    };
    Ok(Some((key, next_child(rd)?)))
}

fn next_child(rd: &mut Reader<&[u8]>) -> Result<Child> {
    loop {
        match rd.read_event().map_err(bad_xml)? {
            Event::Start(e) => return Ok(Child::Open(tag_name(e.name().as_ref()))),
            Event::Empty(e) => return Ok(Child::Empty(tag_name(e.name().as_ref()))),
            Event::End(_) | Event::Eof => return Ok(Child::Closed),
            _ => {}
        }
    }
}

/// Everything between the start tag just read and its close.
fn read_text(rd: &mut Reader<&[u8]>) -> Result<String> {
    let mut out = String::new();
    loop {
        match rd.read_event().map_err(bad_xml)? {
            // 0.42 hands back the escaped source text, so `&amp;` in a
            // track name is still `&amp;` here. CDATA is literal by
            // definition and must not go through the same pass.
            Event::Text(t) => {
                let raw = t.xml10_content();
                out.push_str(&unescape(&raw).map_err(bad_xml)?);
            }
            Event::CData(c) => out.push_str(&c.into_inner()),
            Event::End(_) | Event::Eof => return Ok(out.trim().to_string()),
            _ => {}
        }
    }
}

/// Consumes the element whose start tag was just read, and everything in it.
fn skip_element(rd: &mut Reader<&[u8]>) -> Result<()> {
    let mut depth = 1usize;
    loop {
        match rd.read_event().map_err(bad_xml)? {
            // `Empty` is self-closing and does not change the depth.
            Event::Start(_) => depth += 1,
            Event::End(_) => {
                depth -= 1;
                if depth == 0 {
                    return Ok(());
                }
            }
            Event::Eof => return Ok(()),
            _ => {}
        }
    }
}

fn tag_name(raw: &str) -> String {
    raw.to_string()
}

fn bad_xml(e: impl std::fmt::Display) -> KeystoneError {
    KeystoneError::Other(format!("could not read the Music library XML: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A library with one real file, one streaming entry with no Location, a
    /// folder, Apple's own Music list, and a user playlist holding both of the
    /// tracks. Everything the reader has to tell apart, in one fixture.
    const XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0">
<dict>
  <key>Application Version</key><string>1.5.1</string>
  <key>Tracks</key>
  <dict>
    <key>101</key>
    <dict>
      <key>Track ID</key><integer>101</integer>
      <key>Name</key><string>Local &amp; Loud</string>
      <key>Play Count</key><integer>12</integer>
      <key>Location</key><string>file:///tmp/keystone%20test/a%20track.mp3</string>
    </dict>
    <key>102</key>
    <dict>
      <key>Track ID</key><integer>102</integer>
      <key>Name</key><string>Streaming only</string>
      <key>Apple Music</key><true/>
    </dict>
  </dict>
  <key>Playlists</key>
  <array>
    <dict>
      <key>Name</key><string>Library</string>
      <key>Master</key><true/>
      <key>Playlist Items</key>
      <array><dict><key>Track ID</key><integer>101</integer></dict></array>
    </dict>
    <dict>
      <key>Name</key><string>Music</string>
      <key>Distinguished Kind</key><integer>4</integer>
      <key>Playlist Items</key>
      <array><dict><key>Track ID</key><integer>101</integer></dict></array>
    </dict>
    <dict>
      <key>Name</key><string>Sets</string>
      <key>Folder</key><true/>
    </dict>
    <dict>
      <key>Name</key><string>Chill</string>
      <key>Playlist Items</key>
      <array>
        <dict><key>Track ID</key><integer>101</integer></dict>
        <dict><key>Track ID</key><integer>102</integer></dict>
      </array>
    </dict>
  </array>
</dict>
</plist>
"#;

    #[test]
    fn apples_own_lists_and_folders_are_not_offered_for_import() {
        let parsed = parse(XML).expect("fixture parses");
        let names: Vec<&str> = parsed.playlists.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, vec!["Chill"]);
    }

    #[test]
    fn a_track_with_no_location_is_read_but_contributes_no_file() {
        let parsed = parse(XML).expect("fixture parses");
        assert_eq!(parsed.locations.len(), 1);
        assert!(parsed.locations.contains_key(&101));
        assert!(!parsed.locations.contains_key(&102));
    }

    #[test]
    fn a_location_is_percent_decoded_into_a_path() {
        assert_eq!(
            location_to_path("file:///tmp/keystone%20test/a%20track.mp3"),
            Some(PathBuf::from("/tmp/keystone test/a track.mp3")),
        );
        assert_eq!(
            location_to_path("file://localhost/Users/me/x.aiff"),
            Some(PathBuf::from("/Users/me/x.aiff")),
        );
    }

    /// A network share and an http stream are not files this app can analyse,
    /// and silently turning `//nas/music` into a relative path would have the
    /// import appear to work and find nothing.
    #[test]
    fn a_location_that_is_not_a_local_path_is_refused() {
        assert_eq!(location_to_path("http://example.com/x.mp3"), None);
        assert_eq!(location_to_path("file://nas/share/x.mp3"), None);
    }

    #[test]
    fn an_m3u_resolves_relative_entries_against_its_own_folder() {
        let dir = tempfile::tempdir().expect("tempdir");
        let audio = dir.path().join("one.mp3");
        std::fs::write(&audio, b"not really audio").expect("write");
        let list = dir.path().join("set.m3u");
        std::fs::write(
            &list,
            "#EXTM3U\n#EXTINF:1,One\none.mp3\nmissing.mp3\n/nowhere/two.wav\n",
        )
        .expect("write");

        let found = read_playlist_file(&list).expect("read");
        assert_eq!(found, vec![audio]);
    }
}
