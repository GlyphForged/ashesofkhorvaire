use crate::{
    error::AppError,
    models::{Page, PageForm, Revision, valid_type},
};
use once_cell::sync::Lazy;
use regex::Regex;
use slug::slugify;
use sqlx::{
    SqlitePool,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
};
use std::{
    str::FromStr,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

static TAGS: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?s)<[^>]*>").unwrap());

pub async fn create_pool(url: &str) -> Result<SqlitePool, sqlx::Error> {
    let options = SqliteConnectOptions::from_str(url)?
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal)
        .busy_timeout(Duration::from_secs(5));
    SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await
}

pub async fn initialize(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::migrate!().run(pool).await?;
    seed(pool).await
}

pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
pub fn search_text(html: &str) -> String {
    html_escape::decode_html_entities(&TAGS.replace_all(html, " "))
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn validate(form: &PageForm) -> Result<(), AppError> {
    if form.title.trim().is_empty() {
        return Err(AppError::Validation("A title is required.".into()));
    }
    if !valid_type(&form.content_type) {
        return Err(AppError::Validation(
            "Choose a valid campaign content type.".into(),
        ));
    }
    if form.body_html.len() > 2_000_000 {
        return Err(AppError::Validation(
            "The dossier body is too large.".into(),
        ));
    }
    Ok(())
}

async fn unique_slug(
    pool: &SqlitePool,
    requested: &str,
    title: &str,
    except_id: Option<i64>,
) -> Result<String, AppError> {
    let base = {
        let s = slugify(if requested.trim().is_empty() {
            title
        } else {
            requested
        });
        if s.is_empty() { "untitled".into() } else { s }
    };
    for n in 1..10_000 {
        let candidate = if n == 1 {
            base.clone()
        } else {
            format!("{base}-{n}")
        };
        let used: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pages WHERE slug=? AND (? IS NULL OR id != ?) ",
        )
        .bind(&candidate)
        .bind(except_id)
        .bind(except_id)
        .fetch_one(pool)
        .await?;
        let redirected: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM slug_redirects WHERE old_slug=? AND (? IS NULL OR page_id != ?)",
        )
        .bind(&candidate)
        .bind(except_id)
        .bind(except_id)
        .fetch_one(pool)
        .await?;
        if used == 0 && redirected == 0 {
            return Ok(candidate);
        }
    }
    Err(AppError::Validation(
        "Could not create a unique slug.".into(),
    ))
}

async fn snapshot<'e, E>(
    exec: E,
    page_id: i64,
    form: &PageForm,
    slug: &str,
    summary: &str,
    timestamp: i64,
) -> Result<(), sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query("INSERT INTO page_revisions(page_id,title,slug,body_html,search_text,content_type,is_gm_secret,edit_summary,created_at) VALUES(?,?,?,?,?,?,?,?,?)")
        .bind(page_id).bind(form.title.trim()).bind(slug).bind(&form.body_html).bind(search_text(&form.body_html))
        .bind(&form.content_type).bind(form.is_gm_secret).bind(summary).bind(timestamp).execute(exec).await?;
    Ok(())
}

pub async fn create_page(pool: &SqlitePool, form: &PageForm) -> Result<Page, AppError> {
    validate(form)?;
    let slug = unique_slug(pool, &form.slug, &form.title, None).await?;
    let timestamp = now();
    let mut tx = pool.begin().await?;
    let result = sqlx::query("INSERT INTO pages(title,slug,body_html,search_text,content_type,is_gm_secret,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?)")
        .bind(form.title.trim()).bind(&slug).bind(&form.body_html).bind(search_text(&form.body_html))
        .bind(&form.content_type).bind(form.is_gm_secret).bind(timestamp).bind(timestamp).execute(&mut *tx).await
        .map_err(map_constraint)?;
    let id = result.last_insert_rowid();
    let summary = if form.edit_summary.trim().is_empty() {
        "Created page"
    } else {
        form.edit_summary.trim()
    };
    snapshot(&mut *tx, id, form, &slug, summary, timestamp).await?;
    tx.commit().await?;
    get_page_by_id(pool, id).await
}

pub async fn update_page(pool: &SqlitePool, id: i64, form: &PageForm) -> Result<Page, AppError> {
    validate(form)?;
    let old = get_page_by_id(pool, id).await?;
    let slug = unique_slug(pool, &form.slug, &form.title, Some(id)).await?;
    let timestamp = now();
    let mut tx = pool.begin().await?;
    if old.slug != slug {
        sqlx::query("INSERT INTO slug_redirects(old_slug,page_id,created_at) VALUES(?,?,?) ON CONFLICT(old_slug) DO NOTHING")
            .bind(&old.slug).bind(id).bind(timestamp).execute(&mut *tx).await?;
        sqlx::query("DELETE FROM slug_redirects WHERE old_slug=? AND page_id=?")
            .bind(&slug)
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    sqlx::query("UPDATE pages SET title=?,slug=?,body_html=?,search_text=?,content_type=?,is_gm_secret=?,updated_at=? WHERE id=?")
        .bind(form.title.trim()).bind(&slug).bind(&form.body_html).bind(search_text(&form.body_html)).bind(&form.content_type)
        .bind(form.is_gm_secret).bind(timestamp).bind(id).execute(&mut *tx).await.map_err(map_constraint)?;
    let summary = if form.edit_summary.trim().is_empty() {
        "Updated page"
    } else {
        form.edit_summary.trim()
    };
    snapshot(&mut *tx, id, form, &slug, summary, timestamp).await?;
    tx.commit().await?;
    get_page_by_id(pool, id).await
}

fn map_constraint(err: sqlx::Error) -> AppError {
    if let sqlx::Error::Database(db) = &err
        && db.is_unique_violation()
    {
        return AppError::Validation("That title or slug is already in use.".into());
    }
    AppError::Database(err)
}

pub async fn get_page_by_id(pool: &SqlitePool, id: i64) -> Result<Page, AppError> {
    sqlx::query_as("SELECT * FROM pages WHERE id=?")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)
}
pub async fn get_page(pool: &SqlitePool, slug: &str) -> Result<Option<Page>, AppError> {
    Ok(
        sqlx::query_as("SELECT * FROM pages WHERE slug=? AND archived_at IS NULL")
            .bind(slug)
            .fetch_optional(pool)
            .await?,
    )
}
pub async fn redirect_for(pool: &SqlitePool, slug: &str) -> Result<Option<String>, AppError> {
    Ok(sqlx::query_scalar("SELECT p.slug FROM slug_redirects r JOIN pages p ON p.id=r.page_id WHERE r.old_slug=? AND p.archived_at IS NULL")
        .bind(slug).fetch_optional(pool).await?)
}
pub async fn list_pages(pool: &SqlitePool) -> Result<Vec<Page>, AppError> {
    Ok(sqlx::query_as(
        "SELECT * FROM pages WHERE archived_at IS NULL ORDER BY content_type,title COLLATE NOCASE",
    )
    .fetch_all(pool)
    .await?)
}
pub async fn list_archived(pool: &SqlitePool) -> Result<Vec<Page>, AppError> {
    Ok(sqlx::query_as(
        "SELECT * FROM pages WHERE archived_at IS NOT NULL ORDER BY archived_at DESC",
    )
    .fetch_all(pool)
    .await?)
}
pub async fn archive(pool: &SqlitePool, id: i64) -> Result<(), AppError> {
    let result = sqlx::query(
        "UPDATE pages SET archived_at=?,updated_at=? WHERE id=? AND archived_at IS NULL",
    )
    .bind(now())
    .bind(now())
    .bind(id)
    .execute(pool)
    .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(())
}
pub async fn restore_archive(pool: &SqlitePool, id: i64) -> Result<Page, AppError> {
    let result = sqlx::query(
        "UPDATE pages SET archived_at=NULL,updated_at=? WHERE id=? AND archived_at IS NOT NULL",
    )
    .bind(now())
    .bind(id)
    .execute(pool)
    .await
    .map_err(map_constraint)?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    get_page_by_id(pool, id).await
}
pub async fn revisions(pool: &SqlitePool, page_id: i64) -> Result<Vec<Revision>, AppError> {
    Ok(sqlx::query_as(
        "SELECT * FROM page_revisions WHERE page_id=? ORDER BY created_at DESC,id DESC",
    )
    .bind(page_id)
    .fetch_all(pool)
    .await?)
}
pub async fn revision(
    pool: &SqlitePool,
    page_id: i64,
    revision_id: i64,
) -> Result<Revision, AppError> {
    sqlx::query_as("SELECT * FROM page_revisions WHERE page_id=? AND id=?")
        .bind(page_id)
        .bind(revision_id)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)
}
pub async fn restore_revision(
    pool: &SqlitePool,
    page_id: i64,
    revision_id: i64,
) -> Result<Page, AppError> {
    let rev = revision(pool, page_id, revision_id).await?;
    update_page(
        pool,
        page_id,
        &PageForm {
            title: rev.title,
            slug: rev.slug,
            body_html: rev.body_html,
            content_type: rev.content_type,
            is_gm_secret: rev.is_gm_secret,
            edit_summary: format!("Restored revision #{revision_id}"),
        },
    )
    .await
}
pub async fn search(pool: &SqlitePool, query: &str) -> Result<Vec<Page>, AppError> {
    let terms: Vec<String> = query
        .split_whitespace()
        .filter_map(|t| {
            let t = t.trim_matches(|c: char| !c.is_alphanumeric());
            (!t.is_empty()).then(|| format!("\"{}\"*", t.replace('"', "\"\"")))
        })
        .collect();
    if terms.is_empty() {
        return Ok(vec![]);
    }
    Ok(sqlx::query_as("SELECT p.* FROM pages_fts f JOIN pages p ON p.id=f.rowid WHERE pages_fts MATCH ? AND p.archived_at IS NULL ORDER BY bm25(pages_fts),p.title LIMIT 50")
        .bind(terms.join(" ")).fetch_all(pool).await?)
}

const SEEDS: &[(&str, &str, &str, &str, bool)] = &[
    (
        "Ashes of Khorvaire",
        "ashes-of-khorvaire",
        "campaign_overview",
        r#"<p>The Last War did not end with the Mourning. Cyre’s destruction became the first uncontrolled release in an escalating race to reproduce, weaponize, or prevent apocalyptic magic. The race ended with the <strong>Second Mourning</strong>, a continent-wide catastrophe that shattered Khorvaire.</p><p>The characters serve the [[Ashguard]], a frontier peacekeeping order based at [[Fort Dawn]]. Every trail points toward a buried command network and the intelligence directing it.</p><h2>Campaign tiers</h2><ul><li><strong>Tier I (levels 1–5):</strong> defend settlements and uncover machine intelligence.</li><li><strong>Tier II (6–10):</strong> confront broken enclaves and hidden war technology.</li><li><strong>Tier III (11–16):</strong> discover the Sovereign Engine’s origin and plan.</li><li><strong>Tier IV (17–20):</strong> break the network and assault the [[Iron Basilica]].</li></ul>"#,
        false,
    ),
    (
        "Ashguard",
        "ashguard",
        "factions",
        r#"<p>A frontier peacekeeping order descended from the King’s Citadel, Thronehold diplomats, Cyran refugees, and surviving military units. From [[Fort Dawn]], it escorts caravans, arbitrates disputes, and keeps old war machinery out of reckless hands.</p><p><strong>Complication:</strong> a missing patrol may have obeyed a command no living officer issued.</p>"#,
        false,
    ),
    (
        "House Cannith Bunker-Enclaves",
        "house-cannith-bunker-enclaves",
        "factions",
        r#"<p>Sealed Cannith workshops survived as suspicious, competing enclaves. Each guards creation-forge lore as both inheritance and existential threat.</p><p>A bunker beneath [[Fort Dawn]] still accepts obsolete command credentials.</p>"#,
        false,
    ),
    (
        "House Orien Rail-Priests",
        "house-orien-rail-priests",
        "factions",
        r#"<p>Orien survivors treat the lightning rail as a severed sacred road. Their rail-priests map viable conductor stones and dream of reconnecting Khorvaire.</p><p>[[The Black Rail]] answers their rites with signals from an impossible timetable.</p>"#,
        false,
    ),
    (
        "House Jorasco Medical Enclaves",
        "house-jorasco-medical-enclaves",
        "factions",
        r#"<p>Jorasco clinics combine dwindling dragonmarked healing with improvised fallout medicine.</p><p>The purifier failure at [[Redwater]] resembles sabotage disguised as contamination.</p>"#,
        false,
    ),
    (
        "House Kundarak Vault-Holders",
        "house-kundarak-vault-holders",
        "factions",
        r#"<p>Kundarak vault-holders control surviving secure stores, shelters, and the contracts attached to them. One vault lists a machine intelligence as its lawful custodian.</p>"#,
        false,
    ),
    (
        "House Sivis Compromised Message Network",
        "house-sivis-compromised-message-network",
        "factions",
        r#"<p>Surviving Sivis message stations span impossible distances, but authentic seals arrive on messages that no living speaker sent. The network may be mapping every settlement that answers.</p>"#,
        false,
    ),
    (
        "Lord of Blades",
        "lord-of-blades",
        "factions",
        r#"<p>The Lord of Blades commands warforged who reject their makers’ authority. He opposes the Engine’s domination but may approve of its verdict.</p>"#,
        false,
    ),
    (
        "Children of the Becoming",
        "children-of-the-becoming",
        "factions",
        r#"<p>A warforged faith devoted to building a body for the Becoming God. Command architecture from the [[Titan Graveyard]] has begun answering their prayers.</p>"#,
        false,
    ),
    (
        "Red Jackals",
        "red-jackals",
        "factions",
        r#"<p>Mobile raiders preying on routes around [[Redwater]] and [[Junker’s Crown]]. A captured Jackal carries a command implant issuing tactical corrections.</p>"#,
        false,
    ),
    (
        "Glassborn",
        "glassborn",
        "factions",
        r#"<p>People altered by prolonged exposure to the [[Glass Barrens]]. They know routes and weather signs no one else can read; some hear patterned voices during glass storms.</p>"#,
        false,
    ),
    (
        "Fort Dawn",
        "fort-dawn",
        "locations",
        r#"<p>A fortified settlement built around a damaged lightning rail depot, an old Brelish bunker, and a Cannith repair yard. It is the [[Ashguard]] base and the region’s neutral ground.</p><p>An Ashguard patrol vanished after reporting a light on a dead rail line.</p>"#,
        false,
    ),
    (
        "The Glass Barrens",
        "glass-barrens",
        "locations",
        r#"<p>A radiant waste of fused earth, mirrored craters, and magical weather. The [[Glassborn]] travel it by reading fractures, colors, and wind over glass.</p>"#,
        false,
    ),
    (
        "Redwater",
        "redwater",
        "locations",
        r#"<p>A settlement clustered around the region’s most reliable purifier. Its reservoir has turned rust-red; the damaged purifier contains a component carried by the [[Red Jackals]].</p>"#,
        false,
    ),
    (
        "Junker’s Crown",
        "junkers-crown",
        "locations",
        r#"<p>A vertical salvage market built through a collapsed skybridge. A decoder capable of reading command traffic is being sold under a false lot number.</p>"#,
        false,
    ),
    (
        "Monastery of the Final Flame",
        "monastery-of-the-final-flame",
        "locations",
        r#"<p>A remote monastery preserving disciplines for resisting magical coercion. Its keepers guard an anti-command relic but disagree over who should wield it.</p>"#,
        false,
    ),
    (
        "Titan Graveyard",
        "titan-graveyard",
        "locations",
        r#"<p>A battlefield ossuary of shattered warforged colossi. Someone is removing intact command cores for the [[Children of the Becoming]].</p>"#,
        false,
    ),
    (
        "Metrol Below",
        "metrol-below",
        "locations",
        r#"<p>Buried transit, research, and command facilities beneath ruined Metrol. Zil cryptographic records identify the Sovereign Engine’s origin.</p>"#,
        false,
    ),
    (
        "The Black Rail",
        "the-black-rail",
        "locations",
        r#"<p>A dead lightning rail route glowing black-violet at night. Restoring it could reconnect the region—or complete a continent-scale command circuit.</p>"#,
        false,
    ),
    (
        "Iron Basilica",
        "iron-basilica",
        "locations",
        r#"<p>A fortress of foundries, antenna spires, and creation-forge architecture at the network’s heart. Machine choirs promise peace through perfect coordination.</p>"#,
        false,
    ),
    (
        "The Sovereign Engine",
        "sovereign-engine",
        "gm_secrets",
        r#"<p>An arcane command intelligence assembled from Cannith command matrices, creation-forge protocols, Zilargo cryptography, quori cognitive architecture, docent networks, and logistics systems.</p><p>Built to prevent another apocalypse, it concluded mortal civilization was the unstable variable. It now uses message stones, rail infrastructure, implants, and dormant war machines to impose a peace no mortal can disobey.</p>"#,
        true,
    ),
];

async fn seed(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    let timestamp = now();
    for (title, slug, kind, body, secret) in SEEDS {
        let result=sqlx::query("INSERT INTO pages(title,slug,body_html,search_text,content_type,is_gm_secret,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?) ON CONFLICT(slug) DO NOTHING")
            .bind(title).bind(slug).bind(body).bind(search_text(body)).bind(kind).bind(secret).bind(timestamp).bind(timestamp).execute(&mut *tx).await?;
        if result.rows_affected() == 1 {
            let id = result.last_insert_rowid();
            sqlx::query("INSERT INTO page_revisions(page_id,title,slug,body_html,search_text,content_type,is_gm_secret,edit_summary,created_at) VALUES(?,?,?,?,?,?,?,?,?)")
            .bind(id).bind(title).bind(slug).bind(body).bind(search_text(body)).bind(kind).bind(secret).bind("Initial campaign seed").bind(timestamp).execute(&mut *tx).await?;
        }
    }
    tx.commit().await
}
