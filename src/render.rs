use crate::{db, error::AppError, models::Page};
use html_escape::{encode_double_quoted_attribute, encode_text};
use once_cell::sync::Lazy;
use regex::Regex;
use sqlx::SqlitePool;
use std::collections::{BTreeMap, HashMap, HashSet};

static PARTS: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?is)(<[^>]+>)").unwrap());
static LINK: Lazy<Regex> = Lazy::new(|| Regex::new(r"\[\[([^\[\]]+)\]\]").unwrap());

#[derive(Debug, Clone)]
pub struct BrokenLink {
    pub title: String,
    pub sources: Vec<Page>,
}

impl BrokenLink {
    pub fn create_url(&self) -> String {
        format!("/pages/new?title={}", urlencoding::encode(&self.title))
    }
}

pub fn extract_titles(source: &str) -> Vec<String> {
    let mut titles = Vec::new();
    let mut seen = HashSet::new();
    let mut last = 0;
    let mut skip_depth = 0i32;
    for mat in PARTS.find_iter(source) {
        collect_titles(
            &source[last..mat.start()],
            skip_depth > 0,
            &mut titles,
            &mut seen,
        );
        let lower = mat.as_str().to_ascii_lowercase();
        if ["<script", "<style", "<code", "<pre"]
            .iter()
            .any(|p| lower.starts_with(p))
        {
            skip_depth += 1;
        }
        if ["</script", "</style", "</code", "</pre"]
            .iter()
            .any(|p| lower.starts_with(p))
        {
            skip_depth = (skip_depth - 1).max(0);
        }
        last = mat.end();
    }
    collect_titles(&source[last..], skip_depth > 0, &mut titles, &mut seen);
    titles
}

fn collect_titles(text: &str, skip: bool, titles: &mut Vec<String>, seen: &mut HashSet<String>) {
    if skip {
        return;
    }
    for caps in LINK.captures_iter(text) {
        let title = caps.get(1).unwrap().as_str().trim();
        if title.is_empty() {
            continue;
        }
        let key = title.to_lowercase();
        if seen.insert(key) {
            titles.push(title.to_string());
        }
    }
}

pub async fn linked_pages(pool: &SqlitePool, source: &str) -> Result<Vec<Page>, AppError> {
    let pages = db::list_pages(pool).await?;
    let by_title: HashMap<String, Page> = pages
        .into_iter()
        .map(|p| (p.title.to_lowercase(), p))
        .collect();
    Ok(extract_titles(source)
        .into_iter()
        .filter_map(|t| by_title.get(&t.to_lowercase()).cloned())
        .collect())
}

pub async fn backlinks(pool: &SqlitePool, target: &Page) -> Result<Vec<Page>, AppError> {
    let target_key = target.title.to_lowercase();
    let mut sources: Vec<Page> = db::list_pages(pool)
        .await?
        .into_iter()
        .filter(|p| {
            p.id != target.id
                && extract_titles(&p.body_html)
                    .iter()
                    .any(|t| t.to_lowercase() == target_key)
        })
        .collect();
    sources.sort_by_key(|a| a.title.to_lowercase());
    Ok(sources)
}

pub async fn broken_links(pool: &SqlitePool) -> Result<Vec<BrokenLink>, AppError> {
    let pages = db::list_pages(pool).await?;
    let targets: HashSet<String> = pages.iter().map(|p| p.title.to_lowercase()).collect();
    let mut broken: BTreeMap<String, (String, Vec<Page>)> = BTreeMap::new();
    for page in &pages {
        for title in extract_titles(&page.body_html) {
            let key = title.to_lowercase();
            if !targets.contains(&key) {
                let item = broken.entry(key).or_insert_with(|| (title, Vec::new()));
                item.1.push(page.clone());
            }
        }
    }
    Ok(broken
        .into_values()
        .map(|(title, sources)| BrokenLink { title, sources })
        .collect())
}

pub async fn wiki_html(pool: &SqlitePool, source: &str) -> Result<String, AppError> {
    let pages = db::list_pages(pool).await?;
    let targets: HashMap<String, String> = pages
        .into_iter()
        .map(|p| (p.title.to_lowercase(), p.slug))
        .collect();
    let mut output = String::with_capacity(source.len() + 128);
    let mut last = 0;
    let mut skip_depth = 0i32;
    for mat in PARTS.find_iter(source) {
        render_text(
            &mut output,
            &source[last..mat.start()],
            &targets,
            skip_depth > 0,
        );
        let tag = mat.as_str();
        let lower = tag.to_ascii_lowercase();
        if ["<script", "<style", "<code", "<pre"]
            .iter()
            .any(|p| lower.starts_with(p))
        {
            skip_depth += 1;
        }
        output.push_str(tag);
        if ["</script", "</style", "</code", "</pre"]
            .iter()
            .any(|p| lower.starts_with(p))
        {
            skip_depth = (skip_depth - 1).max(0);
        }
        last = mat.end();
    }
    render_text(&mut output, &source[last..], &targets, skip_depth > 0);
    Ok(output)
}

fn render_text(out: &mut String, text: &str, targets: &HashMap<String, String>, skip: bool) {
    if skip {
        out.push_str(text);
        return;
    }
    let mut last = 0;
    for caps in LINK.captures_iter(text) {
        let full = caps.get(0).unwrap();
        let title = caps.get(1).unwrap().as_str().trim();
        out.push_str(&text[last..full.start()]);
        if let Some(slug) = targets.get(&title.to_lowercase()) {
            out.push_str(&format!(
                r#"<a class="wiki-link" href="/pages/{}">{}</a>"#,
                encode_double_quoted_attribute(slug),
                encode_text(title)
            ));
        } else {
            out.push_str(&format!(r#"<a class="wiki-link missing" href="/pages/new?title={}" title="Missing page">{} <span class="sr-only">(Missing page)</span></a>"#,urlencoding::encode(title),encode_text(title)));
        }
        last = full.end();
    }
    out.push_str(&text[last..]);
}
