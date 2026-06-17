use crate::{
    AppState, db,
    error::AppError,
    models::{CONTENT_TYPES, Page, PageForm, Revision},
    render,
};
use askama::Template;
use axum::{
    Form, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use serde::Deserialize;
use tower_http::{services::ServeDir, trace::TraceLayer};

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/healthz", get(health))
        .route("/pages", get(pages))
        .route("/pages/new", get(new_page).post(create_page))
        .route("/pages/slug-preview", get(slug_preview))
        .route("/pages/{slug}", get(view_page))
        .route("/pages/{slug}/edit", get(edit_page).post(save_page))
        .route("/pages/{slug}/archive", post(archive_page))
        .route("/pages/{slug}/restore", post(restore_page))
        .route("/pages/{slug}/revisions", get(revision_list))
        .route("/pages/{slug}/revisions/{id}", get(view_revision))
        .route(
            "/pages/{slug}/revisions/{id}/restore",
            post(restore_revision),
        )
        .route("/archive", get(archive_list))
        .route("/search", get(search))
        .route("/search/live", get(search_live))
        .nest_service("/static", ServeDir::new("static"))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn health(State(s): State<AppState>) -> Result<StatusCode, AppError> {
    sqlx::query_scalar::<_, i64>("SELECT 1")
        .fetch_one(&s.pool)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

fn html<T: Template>(template: T) -> Result<Html<String>, AppError> {
    Ok(Html(template.render()?))
}
fn see_other(location: String) -> Response {
    (StatusCode::SEE_OTHER, [(header::LOCATION, location)]).into_response()
}
fn is_hx(headers: &HeaderMap) -> bool {
    headers.get("HX-Request").and_then(|v| v.to_str().ok()) == Some("true")
}

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate {
    pages: Vec<Page>,
}
#[derive(Template)]
#[template(path = "pages/list.html")]
struct ListTemplate {
    title: String,
    pages: Vec<Page>,
    archived: bool,
}
#[derive(Template)]
#[template(path = "pages/view.html")]
struct ViewTemplate {
    page: Page,
    rendered: String,
}
#[derive(Template)]
#[template(path = "pages/form.html")]
struct FormTemplate {
    heading: String,
    action: String,
    archive_action: String,
    form: PageForm,
    types: &'static [(&'static str, &'static str)],
    is_edit: bool,
}
#[derive(Template)]
#[template(path = "pages/revisions.html")]
struct RevisionsTemplate {
    page: Page,
    revisions: Vec<Revision>,
}
#[derive(Template)]
#[template(path = "partials/revisions.html")]
struct RevisionsFragment {
    page: Page,
    revisions: Vec<Revision>,
}
#[derive(Template)]
#[template(path = "pages/revision.html")]
struct RevisionTemplate {
    page: Page,
    revision: Revision,
    rendered: String,
}
#[derive(Template)]
#[template(path = "search.html")]
struct SearchTemplate {
    query: String,
    pages: Vec<Page>,
}
#[derive(Template)]
#[template(path = "partials/search_results.html")]
struct SearchResultsTemplate {
    query: String,
    pages: Vec<Page>,
}

async fn index(State(s): State<AppState>) -> Result<Html<String>, AppError> {
    html(IndexTemplate {
        pages: db::list_pages(&s.pool).await?,
    })
}
#[derive(Deserialize, Default)]
struct ListQuery {
    #[serde(rename = "type", default)]
    kind: String,
}
async fn pages(
    State(s): State<AppState>,
    Query(q): Query<ListQuery>,
) -> Result<Html<String>, AppError> {
    let mut items = db::list_pages(&s.pool).await?;
    let title = if q.kind.is_empty() {
        "All dossiers".into()
    } else {
        items.retain(|p| p.content_type == q.kind);
        crate::models::type_label(&q.kind).to_string()
    };
    html(ListTemplate {
        title,
        pages: items,
        archived: false,
    })
}

#[derive(Deserialize, Default)]
struct NewQuery {
    #[serde(default)]
    title: String,
}
async fn new_page(Query(q): Query<NewQuery>) -> Result<Html<String>, AppError> {
    html(FormTemplate {
        heading: "Open a new dossier".into(),
        action: "/pages/new".into(),
        archive_action: String::new(),
        form: PageForm {
            title: q.title,
            content_type: "campaign_overview".into(),
            ..Default::default()
        },
        types: &CONTENT_TYPES,
        is_edit: false,
    })
}
async fn create_page(
    State(s): State<AppState>,
    Form(form): Form<PageForm>,
) -> Result<Response, AppError> {
    let p = db::create_page(&s.pool, &form).await?;
    Ok(see_other(format!("/pages/{}", p.slug)))
}

async fn view_page(
    State(s): State<AppState>,
    Path(slug): Path<String>,
) -> Result<Response, AppError> {
    if let Some(page) = db::get_page(&s.pool, &slug).await? {
        let rendered = render::wiki_html(&s.pool, &page.body_html).await?;
        return Ok(html(ViewTemplate { page, rendered })?.into_response());
    }
    if let Some(current) = db::redirect_for(&s.pool, &slug).await? {
        return Ok(Redirect::permanent(&format!("/pages/{current}")).into_response());
    }
    Err(AppError::NotFound)
}
async fn edit_page(
    State(s): State<AppState>,
    Path(slug): Path<String>,
) -> Result<Html<String>, AppError> {
    let p = db::get_page(&s.pool, &slug)
        .await?
        .ok_or(AppError::NotFound)?;
    let form = PageForm {
        title: p.title.clone(),
        slug: p.slug.clone(),
        body_html: p.body_html.clone(),
        content_type: p.content_type.clone(),
        is_gm_secret: p.is_gm_secret,
        edit_summary: String::new(),
    };
    html(FormTemplate {
        heading: format!("Edit {}", p.title),
        action: format!("/pages/{}/edit", p.slug),
        archive_action: format!("/pages/{}/archive", p.slug),
        form,
        types: &CONTENT_TYPES,
        is_edit: true,
    })
}
async fn save_page(
    State(s): State<AppState>,
    Path(slug): Path<String>,
    Form(form): Form<PageForm>,
) -> Result<Response, AppError> {
    let old = db::get_page(&s.pool, &slug)
        .await?
        .ok_or(AppError::NotFound)?;
    let p = db::update_page(&s.pool, old.id, &form).await?;
    Ok(see_other(format!("/pages/{}?saved=1", p.slug)))
}
async fn archive_page(
    State(s): State<AppState>,
    Path(slug): Path<String>,
) -> Result<Response, AppError> {
    let p = db::get_page(&s.pool, &slug)
        .await?
        .ok_or(AppError::NotFound)?;
    db::archive(&s.pool, p.id).await?;
    Ok(see_other("/archive".into()))
}
async fn restore_page(
    State(s): State<AppState>,
    Path(slug): Path<String>,
) -> Result<Response, AppError> {
    let p: Page = sqlx::query_as("SELECT * FROM pages WHERE slug=? AND archived_at IS NOT NULL")
        .bind(&slug)
        .fetch_optional(&s.pool)
        .await?
        .ok_or(AppError::NotFound)?;
    let restored = db::restore_archive(&s.pool, p.id).await?;
    Ok(see_other(format!("/pages/{}", restored.slug)))
}
async fn archive_list(State(s): State<AppState>) -> Result<Html<String>, AppError> {
    html(ListTemplate {
        title: "Archive".into(),
        pages: db::list_archived(&s.pool).await?,
        archived: true,
    })
}

async fn revision_list(
    State(s): State<AppState>,
    Path(slug): Path<String>,
    headers: HeaderMap,
) -> Result<Html<String>, AppError> {
    let page = db::get_page(&s.pool, &slug)
        .await?
        .ok_or(AppError::NotFound)?;
    let revisions = db::revisions(&s.pool, page.id).await?;
    if is_hx(&headers) {
        html(RevisionsFragment { page, revisions })
    } else {
        html(RevisionsTemplate { page, revisions })
    }
}
async fn view_revision(
    State(s): State<AppState>,
    Path((slug, id)): Path<(String, i64)>,
) -> Result<Html<String>, AppError> {
    let page = db::get_page(&s.pool, &slug)
        .await?
        .ok_or(AppError::NotFound)?;
    let revision = db::revision(&s.pool, page.id, id).await?;
    let rendered = render::wiki_html(&s.pool, &revision.body_html).await?;
    html(RevisionTemplate {
        page,
        revision,
        rendered,
    })
}
async fn restore_revision(
    State(s): State<AppState>,
    Path((slug, id)): Path<(String, i64)>,
) -> Result<Response, AppError> {
    let page = db::get_page(&s.pool, &slug)
        .await?
        .ok_or(AppError::NotFound)?;
    let restored = db::restore_revision(&s.pool, page.id, id).await?;
    Ok(see_other(format!("/pages/{}", restored.slug)))
}

#[derive(Deserialize, Default)]
struct SearchQuery {
    #[serde(default)]
    q: String,
}
async fn search(
    State(s): State<AppState>,
    Query(q): Query<SearchQuery>,
) -> Result<Html<String>, AppError> {
    let pages = db::search(&s.pool, &q.q).await?;
    html(SearchTemplate { query: q.q, pages })
}
async fn search_live(
    State(s): State<AppState>,
    Query(q): Query<SearchQuery>,
) -> Result<Html<String>, AppError> {
    let pages = db::search(&s.pool, &q.q).await?;
    html(SearchResultsTemplate { query: q.q, pages })
}
#[derive(Deserialize, Default)]
struct SlugQuery {
    #[serde(default)]
    title: String,
    #[serde(default)]
    slug: String,
}
async fn slug_preview(Query(q): Query<SlugQuery>) -> Html<String> {
    let slug = if q.slug.trim().is_empty() {
        slug::slugify(q.title)
    } else {
        slug::slugify(q.slug)
    };
    Html(format!("<code>{}</code>", html_escape::encode_text(&slug)))
}
