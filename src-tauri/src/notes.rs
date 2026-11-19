// Release notes: <base>/launcher/notes.json, a public file the server's admin edits by hand or with
// research/tools/deploy/notes-add.sh. Newest first:
//   [{ "id": 7, "date": "2026-11-19", "tag": "servidor", "title": "...", "items": ["...", "..."] }]
// The file comes from the network, so it is read with limits and an entry that does not fit is dropped; a file that
// is missing, too big or not a list gives no notes, and the window then looks as it did before notes existed.
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::time::Duration;

pub const MAX_BYTES: usize = 64 * 1024;
pub const MAX_NOTES: usize = 50;
pub const MAX_ITEMS: usize = 12;
pub const MAX_TITLE: usize = 120; // characters
pub const MAX_ITEM: usize = 400;

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum Tag {
    Servidor,
    Launcher,
    Juego,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct Note {
    pub id: u32,
    pub date: String, // "YYYY-MM-DD", or empty when the file had none or a wrong one
    pub tag: Option<Tag>,
    pub title: String,
    pub items: Vec<String>,
}

#[derive(Deserialize)]
struct Raw {
    id: u32,
    title: String,
    #[serde(default)]
    date: String,
    #[serde(default)]
    tag: Option<serde_json::Value>,
    #[serde(default)]
    items: Vec<serde_json::Value>,
}

fn clip(s: &str, max: usize) -> String {
    let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if s.chars().count() <= max {
        return s;
    }
    s.chars().take(max - 1).collect::<String>() + "…"
}

fn is_date(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 10 && b[4] == b'-' && b[7] == b'-' && b.iter().enumerate().all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
}

fn note(raw: Raw) -> Option<Note> {
    let title = clip(&raw.title, MAX_TITLE);
    if title.is_empty() {
        return None;
    }
    let items = raw.items.iter().filter_map(|v| v.as_str()).map(|s| clip(s, MAX_ITEM)).filter(|s| !s.is_empty()).take(MAX_ITEMS).collect();
    Some(Note {
        id: raw.id,
        date: if is_date(&raw.date) { raw.date } else { String::new() },
        tag: raw.tag.and_then(|t| serde_json::from_value(t).ok()),
        title,
        items,
    })
}

/// The notes in a notes.json, newest (highest id) first, each id once, at most MAX_NOTES.
pub fn parse(text: &str) -> Vec<Note> {
    if text.len() > MAX_BYTES {
        return Vec::new();
    }
    let list: Vec<serde_json::Value> = match serde_json::from_str(text) {
        Ok(l) => l,
        Err(_) => return Vec::new(),
    };
    let mut notes: Vec<Note> = list.into_iter().filter_map(|v| serde_json::from_value::<Raw>(v).ok()).filter_map(note).collect();
    notes.sort_by(|a, b| b.id.cmp(&a.id));
    notes.dedup_by_key(|n| n.id);
    notes.truncate(MAX_NOTES);
    notes
}

/// The server's notes; none when it cannot be reached or the file is wrong.
pub fn fetch() -> Vec<Note> {
    let url = format!("{}/launcher/notes.json", crate::core::base_url());
    let r = (|| -> Result<String, String> {
        let resp = ureq::get(&url).timeout(Duration::from_secs(8)).call().map_err(|e| e.to_string())?;
        let mut buf = Vec::new();
        resp.into_reader().take(MAX_BYTES as u64 + 1).read_to_end(&mut buf).map_err(|e| e.to_string())?;
        if buf.len() > MAX_BYTES {
            return Err("larger than 64 KB".into());
        }
        String::from_utf8(buf).map_err(|e| e.to_string())
    })();
    match r {
        Ok(text) => {
            let notes = parse(&text);
            if notes.is_empty() {
                crate::core::log("notes.json has no usable entries");
            }
            notes
        }
        Err(e) => {
            crate::core::log(&format!("notes not read: {}", e));
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_entries_newest_first() {
        let n = parse(r#"[{"id":1,"date":"2026-11-18","tag":"juego","title":"Uno","items":["a"]},
                          {"id":3,"date":"2026-11-19","tag":"servidor","title":"Tres","items":["b","c"]}]"#);
        assert_eq!(n.iter().map(|n| n.id).collect::<Vec<_>>(), [3, 1]);
        assert_eq!(n[0], Note { id: 3, date: "2026-11-19".into(), tag: Some(Tag::Servidor), title: "Tres".into(), items: vec!["b".into(), "c".into()] });
    }

    #[test]
    fn a_wrong_file_gives_no_notes() {
        for text in ["", "{", "{\"id\":1}", "null", "\"x\"", "<html>404</html>"] {
            assert!(parse(text).is_empty(), "{text}");
        }
    }

    #[test]
    fn a_file_over_64_kb_gives_no_notes() {
        let item = "x".repeat(300);
        let entry = |id: usize| format!(r#"{{"id":{id},"title":"t","items":["{item}"]}}"#);
        let text = format!("[{}]", (1..=300).map(entry).collect::<Vec<_>>().join(","));
        assert!(text.len() > MAX_BYTES);
        assert!(parse(&text).is_empty());
    }

    #[test]
    fn a_bad_entry_is_dropped_and_the_rest_kept() {
        let n = parse(r#"[{"id":"2","title":"id as text"}, {"id":-1,"title":"negative"}, {"id":5,"title":"   "},
                          {"title":"no id"}, 7, {"id":4,"title":"ok"}]"#);
        assert_eq!(n.iter().map(|n| n.id).collect::<Vec<_>>(), [4]);
    }

    #[test]
    fn unknown_tag_or_bad_date_are_left_out_not_the_entry() {
        let n = parse(r#"[{"id":1,"title":"t","tag":"<b>x</b>","date":"19/11/2026"}, {"id":2,"title":"t","tag":3,"date":"2026-11-19x"}]"#);
        assert_eq!(n.len(), 2);
        assert!(n.iter().all(|n| n.tag.is_none() && n.date.is_empty()));
    }

    #[test]
    fn lengths_and_counts_are_capped() {
        let items: Vec<String> = (0..30).map(|i| format!("\"{}{}\"", i, "y".repeat(500))).collect();
        let entries: Vec<String> = (1..=80).map(|id| format!(r#"{{"id":{id},"title":"{}","items":[{}]}}"#, "T".repeat(200), if id == 80 { items.join(",") } else { String::new() })).collect();
        let n = parse(&format!("[{}]", entries.join(",")));
        assert_eq!(n.len(), MAX_NOTES);
        assert_eq!(n[0].id, 80);
        assert_eq!(n[0].title.chars().count(), MAX_TITLE);
        assert!(n[0].title.ends_with('…'));
        assert_eq!(n[0].items.len(), MAX_ITEMS);
        assert!(n[0].items.iter().all(|i| i.chars().count() == MAX_ITEM));
    }

    #[test]
    fn text_is_kept_as_text_with_whitespace_collapsed() {
        let n = parse(r#"[{"id":1,"title":"<img src=x onerror=alert(1)>","items":["a\n\n  b", "", 5, "  "]}]"#);
        assert_eq!(n[0].title, "<img src=x onerror=alert(1)>");
        assert_eq!(n[0].items, ["a b"]);
    }

    #[test]
    fn a_repeated_id_shows_once() {
        let n = parse(r#"[{"id":2,"title":"first"},{"id":2,"title":"second"},{"id":1,"title":"x"}]"#);
        assert_eq!(n.iter().map(|n| n.title.as_str()).collect::<Vec<_>>(), ["first", "x"]);
    }
}
