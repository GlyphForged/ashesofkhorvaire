use ashes_wiki::{AppState, create_pool, db, initialize, models::PageForm, render, router};
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use tempfile::TempDir;
use tower::ServiceExt;

async fn test_pool() -> (TempDir, sqlx::SqlitePool) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("wiki.db");
    let pool = create_pool(&format!("sqlite://{}", path.display()))
        .await
        .unwrap();
    initialize(&pool).await.unwrap();
    (dir, pool)
}
fn form(title: &str, body: &str) -> PageForm {
    PageForm {
        title: title.into(),
        slug: String::new(),
        body_html: body.into(),
        content_type: "locations".into(),
        is_gm_secret: false,
        edit_summary: String::new(),
    }
}

#[tokio::test]
async fn seeds_are_idempotent_and_searchable() {
    let (_dir, pool) = test_pool().await;
    initialize(&pool).await.unwrap();
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM pages")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 21);
    let found = db::search(&pool, "purifier").await.unwrap();
    assert!(found.iter().any(|p| p.title == "Redwater"));
}

#[tokio::test]
async fn create_edit_revision_redirect_and_restore() {
    let (_dir, pool) = test_pool().await;
    let created = db::create_page(&pool, &form("Signal Tower", "<p>First signal</p>"))
        .await
        .unwrap();
    assert_eq!(created.slug, "signal-tower");
    assert_eq!(db::revisions(&pool, created.id).await.unwrap().len(), 1);
    let _updated = db::update_page(
        &pool,
        created.id,
        &PageForm {
            title: "North Signal Tower".into(),
            slug: "north-tower".into(),
            body_html: "<p>Second signal</p>".into(),
            content_type: "locations".into(),
            is_gm_secret: true,
            edit_summary: "Found the dial".into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(
        db::redirect_for(&pool, "signal-tower")
            .await
            .unwrap()
            .as_deref(),
        Some("north-tower")
    );
    let revs = db::revisions(&pool, created.id).await.unwrap();
    assert_eq!(revs.len(), 2);
    let restored = db::restore_revision(&pool, created.id, revs[1].id)
        .await
        .unwrap();
    assert_eq!(restored.title, "Signal Tower");
    assert_eq!(db::revisions(&pool, created.id).await.unwrap().len(), 3);
}

#[tokio::test]
async fn archive_excludes_search_and_can_restore() {
    let (_dir, pool) = test_pool().await;
    let p = db::create_page(&pool, &form("Hidden Pump", "<p>xyzzypump</p>"))
        .await
        .unwrap();
    assert_eq!(db::search(&pool, "xyzzypump").await.unwrap().len(), 1);
    db::archive(&pool, p.id).await.unwrap();
    assert!(db::get_page(&pool, &p.slug).await.unwrap().is_none());
    assert!(db::search(&pool, "xyzzypump").await.unwrap().is_empty());
    db::restore_archive(&pool, p.id).await.unwrap();
    assert!(db::get_page(&pool, &p.slug).await.unwrap().is_some());
}

#[tokio::test]
async fn wiki_links_resolve_and_missing_links_offer_creation() {
    let (_dir, pool) = test_pool().await;
    let html = render::wiki_html(
        &pool,
        "<p>Go to [[Fort Dawn]] then [[Unknown Vault]].</p><code>[[Fort Dawn]]</code>",
    )
    .await
    .unwrap();
    assert!(html.contains("/pages/fort-dawn"));
    assert!(html.contains("/pages/new?title=Unknown%20Vault"));
    assert!(html.contains("<code>[[Fort Dawn]]</code>"));
}

#[tokio::test]
async fn routes_return_pages_redirects_and_friendly_404() {
    let (_dir, pool) = test_pool().await;
    let app = router(AppState { pool });
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/pages/fort-dawn")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = String::from_utf8(
        response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec(),
    )
    .unwrap();
    assert!(body.contains("Fort Dawn"));
    let response = app
        .oneshot(
            Request::builder()
                .uri("/pages/does-not-exist")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
