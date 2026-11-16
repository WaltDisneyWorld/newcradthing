//! On-disk persistence for user settings and the saved-card database.

use crate::model::{SavedCard, Settings};

fn settings_path() -> std::path::PathBuf {
    crate::app::data_dir().join("settings.json")
}

fn cards_path() -> std::path::PathBuf {
    crate::app::data_dir().join("cards.json")
}

pub fn load_settings() -> Settings {
    std::fs::read(settings_path())
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

pub fn save_settings(s: &Settings) {
    ensure_dir();
    if let Ok(json) = serde_json::to_vec_pretty(s) {
        let _ = std::fs::write(settings_path(), json);
    }
}

pub fn load_cards() -> Vec<SavedCard> {
    std::fs::read(cards_path())
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

pub fn save_cards(cards: &[SavedCard]) {
    ensure_dir();
    if let Ok(json) = serde_json::to_vec_pretty(cards) {
        let _ = std::fs::write(cards_path(), json);
    }
}

/// Insert/update a card by hash, keeping the newest capture first.
pub fn upsert(cards: &mut Vec<SavedCard>, card: SavedCard) {
    if let Some(existing) = cards.iter_mut().find(|c| c.hash == card.hash) {
        if !card.name.is_empty() {
            existing.name = card.name;
        }
        return;
    }
    cards.insert(0, card);
    cards.truncate(50);
}

fn ensure_dir() {
    let _ = std::fs::create_dir_all(crate::app::data_dir());
}
