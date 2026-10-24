use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlidePayload {
    pub index: u32,
    pub total: u32,
    pub title: String,
    pub body: String,
    pub notes: String,
    pub background_path: Option<String>,
    pub layout_unsupported: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SlideEntry {
    pub index: u32,
    pub title: String,
    pub body: String,
    pub notes: String,
    pub background_path: Option<String>,
    pub rich_body: Vec<SlideTextBlock>,
    pub word_count: u32,
    pub has_background_image: bool,
    pub is_code_slide: bool,
    pub layout_unsupported: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SlideTextBlock {
    pub runs: Vec<SlideTextRun>,
    pub alignment: TextAlignment,
    pub indent_level: u8,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SlideTextRun {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub font_size_pt: Option<u32>,
    pub color_hex: Option<String>,
    pub font_family: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum TextAlignment {
    Left,
    Center,
    Right,
    Justify,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SlideDeck {
    pub slides: Vec<SlideEntry>,
}

impl SlideDeck {
    pub fn empty() -> Self {
        Self { slides: Vec::new() }
    }

    /// Hardcoded 5-slide stub deck for F1.2. Replaced by file import in F1.4.
    pub fn stub() -> Self {
        Self {
            slides: vec![
                SlideEntry {
                    index: 0,
                    title: "Welcome".into(),
                    body: "Waylamp Present F1.2 State Machine".into(),
                    notes: "Welcome the audience. Mention the event name and your role.".into(),
                    background_path: None,
                    rich_body: Vec::new(),
                    word_count: 0,
                    has_background_image: false,
                    is_code_slide: false,
                    layout_unsupported: false,
                },
                SlideEntry {
                    index: 1,
                    title: "Architecture".into(),
                    body: "Event bus · Rust state · React renderer".into(),
                    notes:
                        "Explain the three-layer architecture: Rust core, Tauri bridge, React UI."
                            .into(),
                    background_path: None,
                    rich_body: Vec::new(),
                    word_count: 0,
                    has_background_image: false,
                    is_code_slide: false,
                    layout_unsupported: false,
                },
                SlideEntry {
                    index: 2,
                    title: "Navigation".into(),
                    body: "Advance · Rewind · Jump · Boundary events".into(),
                    notes: "Demo advance and rewind live. Show the boundary events in the log."
                        .into(),
                    background_path: None,
                    rich_body: Vec::new(),
                    word_count: 0,
                    has_background_image: false,
                    is_code_slide: false,
                    layout_unsupported: false,
                },
                SlideEntry {
                    index: 3,
                    title: "Observability".into(),
                    body: "Every transition is a typed AppEvent".into(),
                    notes: "Point to the event log in the controller — every action is visible."
                        .into(),
                    background_path: None,
                    rich_body: Vec::new(),
                    word_count: 0,
                    has_background_image: false,
                    is_code_slide: false,
                    layout_unsupported: false,
                },
                SlideEntry {
                    index: 4,
                    title: "Next: F1.3".into(),
                    body: "Controller slide panel with thumbnails".into(),
                    notes: "Preview F1.3 features: thumbnails, notes panel, keyboard shortcuts."
                        .into(),
                    background_path: None,
                    rich_body: Vec::new(),
                    word_count: 0,
                    has_background_image: false,
                    is_code_slide: false,
                    layout_unsupported: false,
                },
            ],
        }
    }

    pub fn load_from_json(path: &std::path::Path) -> Result<Self, crate::error::AppError> {
        let data = std::fs::read_to_string(path)
            .map_err(|e| crate::error::AppError::Internal(e.to_string()))?;
        serde_json::from_str(&data).map_err(|e| crate::error::AppError::Internal(e.to_string()))
    }

    pub fn save_to_json(&self, path: &std::path::Path) -> Result<(), crate::error::AppError> {
        let data = serde_json::to_string_pretty(self)
            .map_err(|e| crate::error::AppError::Internal(e.to_string()))?;
        crate::persist::atomic_write(path, data.as_bytes())
    }

    pub fn get(&self, index: usize) -> Option<&SlideEntry> {
        self.slides.get(index)
    }

    pub fn len(&self) -> usize {
        self.slides.len()
    }

    pub fn is_empty(&self) -> bool {
        self.slides.is_empty()
    }
}

impl SlidePayload {
    /// Sentinel for "no slides loaded" — a valid, non-panicking payload with `total: 0`.
    pub fn empty() -> Self {
        Self {
            index: 0,
            total: 0,
            title: String::new(),
            body: String::new(),
            notes: String::new(),
            background_path: None,
            layout_unsupported: false,
        }
    }

    pub fn stub() -> Self {
        Self {
            index: 0,
            total: 1,
            title: "Waylamp Present".into(),
            body: "Ready to present".into(),
            notes: String::new(),
            background_path: None,
            layout_unsupported: false,
        }
    }
}

impl SlidePayload {
    pub fn from_entry(entry: &SlideEntry, total: u32) -> Self {
        Self {
            index: entry.index,
            total,
            title: entry.title.clone(),
            body: entry.body.clone(),
            notes: entry.notes.clone(),
            background_path: entry.background_path.clone(),
            layout_unsupported: entry.layout_unsupported,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stub_has_expected_defaults() {
        let s = SlidePayload::stub();
        assert_eq!(s.index, 0);
        assert_eq!(s.total, 1);
        assert_eq!(s.title, "Waylamp Present");
        assert_eq!(s.body, "Ready to present");
        assert!(s.notes.is_empty());
        assert!(s.background_path.is_none());
    }

    #[test]
    fn round_trip_serialise_deserialise() {
        let original = SlidePayload::stub();
        let json = serde_json::to_string(&original).expect("serialize");
        let restored: SlidePayload = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(original.index, restored.index);
        assert_eq!(original.title, restored.title);
        assert_eq!(original.body, restored.body);
        assert_eq!(original.background_path, restored.background_path);
    }

    #[test]
    fn background_path_serialises_as_null_when_none() {
        let s = SlidePayload::stub();
        let json = serde_json::to_string(&s).expect("serialize");
        assert!(json.contains("\"background_path\":null"));
    }

    #[test]
    fn stub_deck_has_five_slides() {
        assert_eq!(SlideDeck::stub().len(), 5);
    }

    #[test]
    fn from_entry_produces_correct_payload() {
        let deck = SlideDeck::stub();
        let entry = deck.get(2).unwrap();
        let payload = SlidePayload::from_entry(entry, deck.len() as u32);
        assert_eq!(payload.index, 2);
        assert_eq!(payload.total, 5);
        assert_eq!(payload.title, "Navigation");
    }

    #[test]
    fn stub_deck_notes_are_non_empty() {
        let deck = SlideDeck::stub();
        for entry in &deck.slides {
            assert!(
                !entry.notes.is_empty(),
                "Slide {} has empty notes",
                entry.index
            );
        }
    }

    #[test]
    fn slide_entry_new_fields_default_to_safe_values_in_stub() {
        let deck = SlideDeck::stub();
        for entry in &deck.slides {
            assert!(entry.rich_body.is_empty());
            assert_eq!(entry.word_count, 0);
            assert!(!entry.has_background_image);
            assert!(!entry.is_code_slide);
            assert!(!entry.layout_unsupported);
        }
    }

    #[test]
    fn deck_save_and_load_json_round_trips_correctly() {
        let deck = SlideDeck::stub();
        let dir = std::env::temp_dir().join("sd_test_deck");
        let path = dir.join("deck.json");
        deck.save_to_json(&path).unwrap();
        let loaded = SlideDeck::load_from_json(&path).unwrap();
        assert_eq!(deck.slides.len(), loaded.slides.len());
        assert_eq!(deck.slides[0].title, loaded.slides[0].title);
        std::fs::remove_dir_all(&dir).ok();
    }
}
