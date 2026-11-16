//! The artwork catalog: packs of card-face designs shown in the ProCard browser.
//!
//! Packs live under an `assets/artwork/` (or `Resources/Artwork/`) folder beside
//! the executable, indexed by a `catalog.json`. The bundled packs are **original,
//! procedurally generated** abstract designs (see `examples/gen_art.rs`). Drop in
//! your own rights-cleared art by adding a folder + catalog entry — ProCard shows
//! whatever the catalog lists.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::model::Language;

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct Art {
    pub id: String,
    pub name: String,
    /// Path relative to the artwork base directory.
    pub file: String,
    #[serde(default)]
    pub tier: Tier,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum Tier {
    Free,
    Pro,
}

impl Default for Tier {
    fn default() -> Self {
        Tier::Free
    }
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct Pack {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub name_it: String,
    #[serde(default = "default_accent")]
    pub accent: String,
    pub arts: Vec<Art>,
}

fn default_accent() -> String {
    "#3BB4C8".into()
}

impl Pack {
    pub fn display_name(&self, lang: Language) -> &str {
        match lang {
            Language::Italian if !self.name_it.is_empty() => &self.name_it,
            _ => &self.name,
        }
    }

    /// Accent as egui color.
    pub fn accent_color(&self) -> egui::Color32 {
        parse_hex(&self.accent).unwrap_or(egui::Color32::from_rgb(59, 180, 200))
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Catalog {
    #[serde(default)]
    pub packs: Vec<Pack>,
    #[serde(skip)]
    pub base_dir: PathBuf,
}

impl Catalog {
    /// Locate and load the catalog from the first candidate directory found.
    pub fn load() -> Self {
        for dir in candidate_dirs() {
            let index = dir.join("catalog.json");
            if let Ok(bytes) = std::fs::read(&index) {
                if let Ok(mut cat) = serde_json::from_slice::<Catalog>(&bytes) {
                    cat.base_dir = dir;
                    return cat;
                }
            }
        }
        Catalog::default()
    }

    pub fn is_empty(&self) -> bool {
        self.packs.is_empty()
    }

    pub fn total_arts(&self) -> usize {
        self.packs.iter().map(|p| p.arts.len()).sum()
    }

    pub fn art_path(&self, art: &Art) -> PathBuf {
        self.base_dir.join(&art.file)
    }

    /// Find an art by id across all packs.
    pub fn find(&self, art_id: &str) -> Option<(&Pack, &Art)> {
        for p in &self.packs {
            if let Some(a) = p.arts.iter().find(|a| a.id == art_id) {
                return Some((p, a));
            }
        }
        None
    }
}

fn candidate_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![
        PathBuf::from("assets/artwork"),
        PathBuf::from("Resources/Artwork"),
    ];
    if let Ok(exe) = std::env::current_exe() {
        if let Some(d) = exe.parent() {
            dirs.push(d.join("assets/artwork"));
            dirs.push(d.join("Resources/Artwork"));
            // one level up (target/release/../../assets during dev)
            if let Some(dd) = d.parent().and_then(Path::parent) {
                dirs.push(dd.join("assets/artwork"));
            }
        }
    }
    dirs
}

fn parse_hex(s: &str) -> Option<egui::Color32> {
    let s = s.trim_start_matches('#');
    if s.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&s[0..2], 16).ok()?;
    let g = u8::from_str_radix(&s[2..4], 16).ok()?;
    let b = u8::from_str_radix(&s[4..6], 16).ok()?;
    Some(egui::Color32::from_rgb(r, g, b))
}
