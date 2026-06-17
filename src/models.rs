use serde::{Deserialize, Serialize};
use sqlx::FromRow;

pub const CONTENT_TYPES: [(&str, &str); 12] = [
    ("campaign_overview", "Campaign Overview"),
    ("settlements", "Settlements"),
    ("locations", "Locations"),
    ("factions", "Factions"),
    ("npcs", "NPCs"),
    ("quests", "Quests / Adventures"),
    ("timeline", "Timeline"),
    ("items", "Items / Artifacts"),
    ("monsters", "Monsters / Encounters"),
    ("session_notes", "Session Notes"),
    ("gm_secrets", "GM Secrets"),
    ("handouts", "Player-Facing Handouts"),
];

pub fn type_label(value: &str) -> &'static str {
    CONTENT_TYPES
        .iter()
        .find(|(key, _)| *key == value)
        .map(|(_, label)| *label)
        .unwrap_or("Unknown")
}
pub fn valid_type(value: &str) -> bool {
    CONTENT_TYPES.iter().any(|(key, _)| *key == value)
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct Page {
    pub id: i64,
    pub title: String,
    pub slug: String,
    pub body_html: String,
    pub search_text: String,
    pub content_type: String,
    pub is_gm_secret: bool,
    pub created_at: i64,
    pub updated_at: i64,
    pub archived_at: Option<i64>,
}
impl Page {
    pub fn type_label(&self) -> &'static str {
        type_label(&self.content_type)
    }
}

#[derive(Debug, Clone, FromRow)]
pub struct Revision {
    pub id: i64,
    pub page_id: i64,
    pub title: String,
    pub slug: String,
    pub body_html: String,
    pub search_text: String,
    pub content_type: String,
    pub is_gm_secret: bool,
    pub edit_summary: String,
    pub created_at: i64,
}
impl Revision {
    pub fn type_label(&self) -> &'static str {
        type_label(&self.content_type)
    }
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct PageForm {
    pub title: String,
    pub slug: String,
    pub body_html: String,
    pub content_type: String,
    #[serde(default)]
    pub is_gm_secret: bool,
    #[serde(default)]
    pub edit_summary: String,
}
