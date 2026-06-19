use ashes_wiki::{
    AppState,
    content::{self, ContentPack, ContentPage},
    create_pool, db, initialize,
    models::PageForm,
    render, router,
};
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

fn session_form(title: &str, body: &str) -> PageForm {
    PageForm {
        title: title.into(),
        slug: String::new(),
        body_html: body.into(),
        content_type: "session_notes".into(),
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

#[tokio::test]
async fn health_route_checks_the_database() {
    let (_dir, pool) = test_pool().await;
    let response = router(AppState { pool })
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn active_session_replaces_clears_and_records_quick_notes() {
    let (_dir, pool) = test_pool().await;
    let first = db::create_page(&pool, &session_form("Session One", "<p>Opening</p>"))
        .await
        .unwrap();
    let second = db::create_page(
        &pool,
        &session_form("Session Two", "<p>Meet [[Fort Dawn]]</p>"),
    )
    .await
    .unwrap();

    db::set_active_session(&pool, first.id).await.unwrap();
    assert_eq!(
        db::active_session(&pool).await.unwrap().unwrap().id,
        first.id
    );
    db::set_active_session(&pool, second.id).await.unwrap();
    assert_eq!(
        db::active_session(&pool).await.unwrap().unwrap().id,
        second.id
    );

    let updated =
        db::append_session_note(&pool, "Ambush <script>alert(1)</script>\nPatrol escaped")
            .await
            .unwrap();
    assert!(updated.body_html.contains("&lt;script&gt;"));
    assert!(updated.body_html.contains("<br>Patrol escaped"));
    let revisions = db::revisions(&pool, second.id).await.unwrap();
    assert_eq!(revisions[0].edit_summary, "Quick note added");

    db::archive(&pool, second.id).await.unwrap();
    assert!(db::active_session(&pool).await.unwrap().is_none());
    let fort = db::get_page(&pool, "fort-dawn").await.unwrap().unwrap();
    assert!(db::set_active_session(&pool, fort.id).await.is_err());
}

#[tokio::test]
async fn retyping_active_session_clears_the_setting() {
    let (_dir, pool) = test_pool().await;
    let session = db::create_page(&pool, &session_form("Retyped Session", "<p>Notes</p>"))
        .await
        .unwrap();
    db::set_active_session(&pool, session.id).await.unwrap();
    db::update_page(
        &pool,
        session.id,
        &form("Retyped Session", "<p>Now a place</p>"),
    )
    .await
    .unwrap();
    assert!(db::active_session(&pool).await.unwrap().is_none());
}

#[tokio::test]
async fn link_analysis_drives_session_links_backlinks_and_diagnostics() {
    let (_dir, pool) = test_pool().await;
    let source = db::create_page(
        &pool,
        &session_form(
            "Linked Session",
            "<p>Visit [[Fort Dawn]], then [[Unwritten Vault]].</p><code>[[Ignored Link]]</code>",
        ),
    )
    .await
    .unwrap();

    let linked = render::linked_pages(&pool, &source.body_html)
        .await
        .unwrap();
    assert_eq!(
        linked.iter().map(|p| p.title.as_str()).collect::<Vec<_>>(),
        vec!["Fort Dawn"]
    );
    let fort = db::get_page(&pool, "fort-dawn").await.unwrap().unwrap();
    let backlinks = render::backlinks(&pool, &fort).await.unwrap();
    assert!(backlinks.iter().any(|page| page.id == source.id));
    let broken = render::broken_links(&pool).await.unwrap();
    let vault = broken
        .iter()
        .find(|link| link.title == "Unwritten Vault")
        .unwrap();
    assert!(vault.sources.iter().any(|page| page.id == source.id));
    assert!(!broken.iter().any(|link| link.title == "Ignored Link"));
}

#[tokio::test]
async fn content_packs_are_idempotent_detect_conflicts_and_preserve_local_pages() {
    let (_dir, pool) = test_pool().await;
    let local = db::create_page(
        &pool,
        &session_form("Pi-only Session", "<p>Do not overwrite me.</p>"),
    )
    .await
    .unwrap();
    let mut pack = ContentPack {
        format: content::CONTENT_PACK_FORMAT,
        pack: "test-campaign".into(),
        pages: vec![ContentPage {
            title: "Managed Person".into(),
            slug: "managed-person".into(),
            body_html: "<p>First version</p>".into(),
            content_type: "npcs".into(),
            is_gm_secret: false,
        }],
    };

    let first = content::apply_pack(&pool, &pack, false).await.unwrap();
    assert_eq!(first.created, 1);
    let managed = db::get_page(&pool, "managed-person")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(db::revisions(&pool, managed.id).await.unwrap().len(), 1);

    let second = content::apply_pack(&pool, &pack, false).await.unwrap();
    assert_eq!(second.unchanged, 1);
    assert_eq!(db::revisions(&pool, managed.id).await.unwrap().len(), 1);

    db::update_page(
        &pool,
        managed.id,
        &PageForm {
            title: managed.title.clone(),
            slug: managed.slug.clone(),
            body_html: "<p>Edited directly on the Pi</p>".into(),
            content_type: managed.content_type.clone(),
            is_gm_secret: managed.is_gm_secret,
            edit_summary: "Local edit".into(),
        },
    )
    .await
    .unwrap();
    pack.pages[0].body_html = "<p>Second version</p>".into();

    let conflict = content::apply_pack(&pool, &pack, false)
        .await
        .unwrap_err()
        .to_string();
    assert!(conflict.contains("managed-person has local changes"));
    assert_eq!(
        db::get_page(&pool, "managed-person")
            .await
            .unwrap()
            .unwrap()
            .body_html,
        "<p>Edited directly on the Pi</p>"
    );

    let forced = content::apply_pack(&pool, &pack, true).await.unwrap();
    assert_eq!(forced.updated, 1);
    assert_eq!(
        db::get_page_by_id(&pool, local.id).await.unwrap().body_html,
        "<p>Do not overwrite me.</p>"
    );
}
