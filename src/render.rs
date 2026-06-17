use crate::{db, error::AppError};
use html_escape::{encode_double_quoted_attribute, encode_text};
use once_cell::sync::Lazy;
use regex::Regex;
use sqlx::SqlitePool;
use std::collections::HashMap;

static PARTS: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?is)(<[^>]+>)").unwrap());
static LINK: Lazy<Regex> = Lazy::new(|| Regex::new(r"\[\[([^\[\]]+)\]\]").unwrap());

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
