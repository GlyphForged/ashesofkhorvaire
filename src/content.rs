use crate::{
    db,
    models::{Page, PageForm, valid_type},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use std::{collections::HashSet, error::Error, fmt::Write, fs, path::Path};

pub const CONTENT_PACK_FORMAT: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentPack {
    pub format: u32,
    pub pack: String,
    pub pages: Vec<ContentPage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentPage {
    pub title: String,
    pub slug: String,
    pub body_html: String,
    pub content_type: String,
    #[serde(default)]
    pub is_gm_secret: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ApplyReport {
    pub created: usize,
    pub updated: usize,
    pub unchanged: usize,
}

pub fn read_pack(path: impl AsRef<Path>) -> Result<ContentPack, Box<dyn Error>> {
    let data = fs::read_to_string(path)?;
    let pack = serde_json::from_str(&data)?;
    Ok(pack)
}

pub fn write_pack(path: impl AsRef<Path>, pack: &ContentPack) -> Result<(), Box<dyn Error>> {
    let data = serde_json::to_string_pretty(pack)?;
    fs::write(path, format!("{data}\n"))?;
    Ok(())
}

pub async fn export_pack(
    pool: &SqlitePool,
    pack_name: &str,
) -> Result<ContentPack, Box<dyn Error>> {
    let pages: Vec<Page> = sqlx::query_as(
        "SELECT * FROM pages WHERE archived_at IS NULL AND content_type != 'session_notes' ORDER BY content_type, title COLLATE NOCASE",
    )
    .fetch_all(pool)
    .await?;
    Ok(ContentPack {
        format: CONTENT_PACK_FORMAT,
        pack: pack_name.to_string(),
        pages: pages
            .into_iter()
            .map(|page| ContentPage {
                title: page.title,
                slug: page.slug,
                body_html: page.body_html,
                content_type: page.content_type,
                is_gm_secret: page.is_gm_secret,
            })
            .collect(),
    })
}

pub async fn apply_pack(
    pool: &SqlitePool,
    pack: &ContentPack,
    force: bool,
) -> Result<ApplyReport, Box<dyn Error>> {
    check_pack(pool, pack, force).await?;
    let mut report = ApplyReport::default();
    for incoming in &pack.pages {
        let existing = page_by_slug(pool, &incoming.slug).await?;
        let incoming_hash = page_hash(incoming)?;
        match existing {
            Some(page) => {
                if page.archived_at.is_some() {
                    db::restore_archive(pool, page.id).await?;
                }
                if stored_page_hash(&page)? == incoming_hash {
                    report.unchanged += 1;
                } else {
                    db::update_page(
                        pool,
                        page.id,
                        &PageForm {
                            title: incoming.title.clone(),
                            slug: incoming.slug.clone(),
                            body_html: incoming.body_html.clone(),
                            content_type: incoming.content_type.clone(),
                            is_gm_secret: incoming.is_gm_secret,
                            edit_summary: format!("Applied content pack {}", pack.pack),
                        },
                    )
                    .await?;
                    report.updated += 1;
                }
            }
            None => {
                db::create_page(
                    pool,
                    &PageForm {
                        title: incoming.title.clone(),
                        slug: incoming.slug.clone(),
                        body_html: incoming.body_html.clone(),
                        content_type: incoming.content_type.clone(),
                        is_gm_secret: incoming.is_gm_secret,
                        edit_summary: format!("Created by content pack {}", pack.pack),
                    },
                )
                .await?;
                report.created += 1;
            }
        }
        sqlx::query(
            "INSERT INTO content_pack_state(pack,slug,applied_hash,applied_at) VALUES(?,?,?,?) ON CONFLICT(pack,slug) DO UPDATE SET applied_hash=excluded.applied_hash,applied_at=excluded.applied_at",
        )
        .bind(&pack.pack)
        .bind(&incoming.slug)
        .bind(&incoming_hash)
        .bind(db::now())
        .execute(pool)
        .await?;
    }
    Ok(report)
}

pub async fn check_pack(
    pool: &SqlitePool,
    pack: &ContentPack,
    force: bool,
) -> Result<(), Box<dyn Error>> {
    validate_pack(pack)?;
    let mut conflicts = Vec::new();

    for incoming in &pack.pages {
        let current = page_by_slug(pool, &incoming.slug).await?;
        let incoming_hash = page_hash(incoming)?;
        let last_hash: Option<String> = sqlx::query_scalar(
            "SELECT applied_hash FROM content_pack_state WHERE pack=? AND slug=?",
        )
        .bind(&pack.pack)
        .bind(&incoming.slug)
        .fetch_optional(pool)
        .await?;

        match current {
            Some(ref page) if page.archived_at.is_some() && !force => {
                conflicts.push(format!("{} is archived", incoming.slug));
            }
            Some(ref page) => {
                let title_owner: Option<i64> = sqlx::query_scalar(
                    "SELECT id FROM pages WHERE title=? COLLATE NOCASE AND archived_at IS NULL AND id != ?",
                )
                .bind(&incoming.title)
                .bind(page.id)
                .fetch_optional(pool)
                .await?;
                if title_owner.is_some() {
                    conflicts.push(format!(
                        "{} has a title owned by another page",
                        incoming.slug
                    ));
                    continue;
                }
                let current_hash = stored_page_hash(page)?;
                let locally_changed = last_hash.as_deref().is_none_or(|hash| hash != current_hash);
                if locally_changed && current_hash != incoming_hash && !force {
                    conflicts.push(format!("{} has local changes", incoming.slug));
                }
            }
            None => {
                let title_owner: Option<String> = sqlx::query_scalar(
                    "SELECT slug FROM pages WHERE title=? COLLATE NOCASE AND archived_at IS NULL",
                )
                .bind(&incoming.title)
                .fetch_optional(pool)
                .await?;
                if let Some(slug) = title_owner {
                    conflicts.push(format!(
                        "{} conflicts with title owned by slug {slug}",
                        incoming.slug
                    ));
                }
            }
        }
    }

    if !conflicts.is_empty() {
        return Err(format!(
            "content pack has conflicts:\n  - {}\nRe-run with --force only after reviewing them.",
            conflicts.join("\n  - ")
        )
        .into());
    }
    Ok(())
}

fn validate_pack(pack: &ContentPack) -> Result<(), Box<dyn Error>> {
    if pack.format != CONTENT_PACK_FORMAT {
        return Err(format!(
            "unsupported content pack format {}; expected {}",
            pack.format, CONTENT_PACK_FORMAT
        )
        .into());
    }
    if pack.pack.trim().is_empty() {
        return Err("content pack name cannot be empty".into());
    }
    let mut slugs = HashSet::new();
    let mut titles = HashSet::new();
    for page in &pack.pages {
        if page.title.trim().is_empty() || page.slug.trim().is_empty() {
            return Err("content pages require a title and slug".into());
        }
        if !valid_type(&page.content_type) {
            return Err(format!("{} has invalid content type", page.slug).into());
        }
        if !slugs.insert(page.slug.to_lowercase()) {
            return Err(format!("duplicate slug in pack: {}", page.slug).into());
        }
        if !titles.insert(page.title.to_lowercase()) {
            return Err(format!("duplicate title in pack: {}", page.title).into());
        }
    }
    Ok(())
}

async fn page_by_slug(pool: &SqlitePool, slug: &str) -> Result<Option<Page>, sqlx::Error> {
    sqlx::query_as("SELECT * FROM pages WHERE slug=?")
        .bind(slug)
        .fetch_optional(pool)
        .await
}

fn stored_page_hash(page: &Page) -> Result<String, serde_json::Error> {
    page_hash(&ContentPage {
        title: page.title.clone(),
        slug: page.slug.clone(),
        body_html: page.body_html.clone(),
        content_type: page.content_type.clone(),
        is_gm_secret: page.is_gm_secret,
    })
}

fn page_hash(page: &ContentPage) -> Result<String, serde_json::Error> {
    let bytes = serde_json::to_vec(page)?;
    let digest = Sha256::digest(bytes);
    let mut hash = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(&mut hash, "{byte:02x}").expect("writing to a String cannot fail");
    }
    Ok(hash)
}
