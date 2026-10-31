//! The streamed `DJ_PLAYLISTS` document.
//!
//! Every element is written as it is produced and nothing is retained, because
//! a 5,000-track collection runs to tens of MB: a DOM would hold the whole
//! library in memory to say the same thing.

use std::io::{BufWriter, Write};
use std::path::Path;

use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, Event};
use quick_xml::{Reader, Writer};

use super::{hot_cue_num, kind_attr, mark_type, rating_attr, tempo_markers, tonality};
use super::{location_uri, PLAYLIST_NAME};
use crate::error::{KeystoneError, Result};
use crate::export::label::cue_label;
use crate::export::{safety, ExportTrack};
use crate::models::CuePoint;

/// The format version the document declares. AlphaTheta's own spec document.
const FORMAT_VERSION: &str = "1.0.0";

pub fn write_document(dest: &Path, tracks: &[&ExportTrack]) -> Result<usize> {
    safety::atomic_write(dest, |file| {
        let mut w = Writer::new_with_indent(BufWriter::new(file), b' ', 2);
        w.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))
            .map_err(io)?;
        let mut root = BytesStart::new("DJ_PLAYLISTS");
        root.push_attribute(("Version", FORMAT_VERSION));
        w.write_event(Event::Start(root)).map_err(io)?;

        let mut product = BytesStart::new("PRODUCT");
        product.push_attribute(("Name", "Keystone"));
        product.push_attribute(("Version", env!("CARGO_PKG_VERSION")));
        product.push_attribute(("Company", "Swendl"));
        w.write_event(Event::Empty(product)).map_err(io)?;

        write_collection(&mut w, tracks)?;
        write_playlists(&mut w, tracks.len())?;

        w.write_event(Event::End(BytesEnd::new("DJ_PLAYLISTS")))
            .map_err(io)?;
        w.into_inner().flush()?;
        Ok(())
    })?;
    Ok(tracks.len())
}

fn write_collection<W: Write>(w: &mut Writer<W>, tracks: &[&ExportTrack]) -> Result<()> {
    let mut collection = BytesStart::new("COLLECTION");
    collection.push_attribute(("Entries", tracks.len().to_string().as_str()));
    w.write_event(Event::Start(collection)).map_err(io)?;
    for (i, track) in tracks.iter().enumerate() {
        write_track(w, track, track_id(i))?;
    }
    w.write_event(Event::End(BytesEnd::new("COLLECTION")))
        .map_err(io)?;
    Ok(())
}

/// The playlist references tracks by `TrackID` (`KeyType="0"`), so the ids are
/// ours to assign and are just the 1-based position in the collection.
fn track_id(index: usize) -> String {
    (index + 1).to_string()
}

fn write_track<W: Write>(w: &mut Writer<W>, track: &ExportTrack, id: String) -> Result<()> {
    w.write_event(Event::Start(track_element(track, &id))).map_err(io)?;
    for m in tempo_markers(track) {
        let mut tempo = BytesStart::new("TEMPO");
        tempo.push_attribute(("Inizio", format!("{:.3}", m.inizio_s).as_str()));
        tempo.push_attribute(("Bpm", format!("{:.2}", m.bpm).as_str()));
        tempo.push_attribute(("Metro", m.metro.as_str()));
        tempo.push_attribute(("Battito", m.battito.to_string().as_str()));
        w.write_event(Event::Empty(tempo)).map_err(io)?;
    }
    for cue in &track.cues {
        write_position_mark(w, cue)?;
    }
    w.write_event(Event::End(BytesEnd::new("TRACK"))).map_err(io)?;
    Ok(())
}

/// `Location` is the attribute that decides whether rekordbox finds the file at
/// all, which is why it is written for every track even when everything else is
/// unknown.
fn track_element<'a>(track: &'a ExportTrack, id: &'a str) -> BytesStart<'a> {
    let t = &track.track;
    let mut e = BytesStart::new("TRACK");
    e.push_attribute(("TrackID", id));
    e.push_attribute(("Name", t.title.as_deref().unwrap_or(&t.filename)));
    e.push_attribute(("Artist", t.artist.as_deref().unwrap_or("")));
    e.push_attribute(("Album", t.album.as_deref().unwrap_or("")));
    e.push_attribute(("Genre", t.genre.as_deref().unwrap_or("")));
    e.push_attribute(("Grouping", t.grouping.as_deref().unwrap_or("")));
    e.push_attribute(("Comments", t.comment.as_deref().unwrap_or("")));
    e.push_attribute(("Kind", kind_attr(&t.extension).as_str()));
    push_numbers(&mut e, track);
    if !track.key_string.is_empty() {
        e.push_attribute(("Tonality", tonality(track).as_str()));
    }
    e.push_attribute(("Location", location_uri(Path::new(&t.path)).as_str()));
    e
}

fn push_numbers(e: &mut BytesStart<'_>, track: &ExportTrack) {
    let t = &track.track;
    if let Some(size) = t.size_bytes {
        e.push_attribute(("Size", size.to_string().as_str()));
    }
    // TotalTime is whole seconds in rekordbox's own exports.
    if let Some(ms) = t.duration_ms {
        let seconds = (ms as f64 / 1000.0).round() as i64;
        e.push_attribute(("TotalTime", seconds.to_string().as_str()));
    }
    if let Some(bpm) = t.tempo_display.or(t.tempo_raw) {
        e.push_attribute(("AverageBpm", format!("{bpm:.2}").as_str()));
    }
    if let Some(year) = t.year {
        e.push_attribute(("Year", year.to_string().as_str()));
    }
    if let Some(br) = t.bitrate_kbps {
        e.push_attribute(("BitRate", br.to_string().as_str()));
    }
    if let Some(sr) = t.sample_rate {
        e.push_attribute(("SampleRate", sr.to_string().as_str()));
    }
    e.push_attribute(("PlayCount", t.play_count.to_string().as_str()));
    e.push_attribute((
        "Rating",
        rating_attr(t.rating.unwrap_or(0)).to_string().as_str(),
    ));
}

fn write_position_mark<W: Write>(w: &mut Writer<W>, cue: &CuePoint) -> Result<()> {
    let mut e = BytesStart::new("POSITION_MARK");
    e.push_attribute(("Name", cue_label(cue).as_str()));
    let is_loop = cue.end_ms.is_some();
    let kind = if is_loop { mark_type::LOOP } else { mark_type::CUE };
    e.push_attribute(("Type", kind.to_string().as_str()));
    e.push_attribute((
        "Start",
        format!("{:.3}", cue.position_ms / 1000.0).as_str(),
    ));
    if let Some(end) = cue.end_ms {
        e.push_attribute(("End", format!("{:.3}", end / 1000.0).as_str()));
    }
    e.push_attribute(("Num", hot_cue_num(cue.slot).to_string().as_str()));
    // Red/Green/Blue carry the hot-cue colour and are **undocumented**:
    // community-verified only, and absent from AlphaTheta's spec. They are
    // written only when we actually have a colour, so a future rekordbox that
    // rejects them can be met by dropping these three lines.
    if let Some((r, g, b)) = cue.color.as_deref().and_then(parse_hex_rgb) {
        e.push_attribute(("Red", r.to_string().as_str()));
        e.push_attribute(("Green", g.to_string().as_str()));
        e.push_attribute(("Blue", b.to_string().as_str()));
    }
    w.write_event(Event::Empty(e)).map_err(io)?;
    Ok(())
}

fn parse_hex_rgb(hex: &str) -> Option<(u8, u8, u8)> {
    let h = hex.trim_start_matches('#');
    if h.len() != 6 {
        return None;
    }
    Some((
        u8::from_str_radix(&h[0..2], 16).ok()?,
        u8::from_str_radix(&h[2..4], 16).ok()?,
        u8::from_str_radix(&h[4..6], 16).ok()?,
    ))
}

/// `PLAYLISTS` -> root `NODE` -> one named node holding every track.
fn write_playlists<W: Write>(w: &mut Writer<W>, count: usize) -> Result<()> {
    w.write_event(Event::Start(BytesStart::new("PLAYLISTS")))
        .map_err(io)?;
    let mut root = BytesStart::new("NODE");
    root.push_attribute(("Type", "0"));
    root.push_attribute(("Name", "ROOT"));
    root.push_attribute(("Count", "1"));
    w.write_event(Event::Start(root)).map_err(io)?;

    let mut node = BytesStart::new("NODE");
    node.push_attribute(("Name", PLAYLIST_NAME));
    node.push_attribute(("Type", "1"));
    // KeyType 0 means the entries below reference TrackID.
    node.push_attribute(("KeyType", "0"));
    node.push_attribute(("Entries", count.to_string().as_str()));
    w.write_event(Event::Start(node)).map_err(io)?;
    for i in 0..count {
        let mut entry = BytesStart::new("TRACK");
        entry.push_attribute(("Key", track_id(i).as_str()));
        w.write_event(Event::Empty(entry)).map_err(io)?;
    }
    w.write_event(Event::End(BytesEnd::new("NODE"))).map_err(io)?;
    w.write_event(Event::End(BytesEnd::new("NODE"))).map_err(io)?;
    w.write_event(Event::End(BytesEnd::new("PLAYLISTS")))
        .map_err(io)?;
    Ok(())
}

/// Re-parses what was written. A file that cannot be read back, or that lost
/// elements on the way out, is reported now rather than by rekordbox refusing
/// the import with no explanation.
pub fn verify_document(dest: &Path, expect_tracks: usize, expect_marks: usize) -> Result<usize> {
    let text = std::fs::read_to_string(dest)?;
    let mut reader = Reader::from_str(&text);
    let (mut in_collection, mut tracks, mut marks) = (false, 0usize, 0usize);
    loop {
        match reader
            .read_event()
            .map_err(|e| KeystoneError::Export(format!("the file Keystone wrote will not parse: {e}")))?
        {
            Event::Start(e) if e.name().as_ref() == "COLLECTION" => in_collection = true,
            Event::End(e) if e.name().as_ref() == "COLLECTION" => in_collection = false,
            Event::Start(e) if in_collection && e.name().as_ref() == "TRACK" => tracks += 1,
            Event::Empty(e) if e.name().as_ref() == "POSITION_MARK" => marks += 1,
            Event::Eof => break,
            _ => {}
        }
    }
    if tracks != expect_tracks || marks != expect_marks {
        return Err(KeystoneError::Export(format!(
            "verification failed: wrote {tracks} tracks and {marks} cues, expected \
             {expect_tracks} and {expect_marks}"
        )));
    }
    Ok(tracks)
}

fn io(e: std::io::Error) -> KeystoneError {
    KeystoneError::Io(e)
}
