PRAGMA foreign_keys = ON;

CREATE TABLE pages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    title TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE,
    body_html TEXT NOT NULL,
    search_text TEXT NOT NULL,
    content_type TEXT NOT NULL CHECK (content_type IN (
        'campaign_overview','settlements','locations','factions','npcs','quests',
        'timeline','items','monsters','session_notes','gm_secrets','handouts'
    )),
    is_gm_secret INTEGER NOT NULL DEFAULT 0 CHECK (is_gm_secret IN (0,1)),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    archived_at INTEGER
);

CREATE UNIQUE INDEX active_page_title_unique
ON pages(title COLLATE NOCASE) WHERE archived_at IS NULL;
CREATE INDEX pages_type_active ON pages(content_type, archived_at, title);

CREATE TABLE page_revisions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    page_id INTEGER NOT NULL REFERENCES pages(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    slug TEXT NOT NULL,
    body_html TEXT NOT NULL,
    search_text TEXT NOT NULL,
    content_type TEXT NOT NULL,
    is_gm_secret INTEGER NOT NULL,
    edit_summary TEXT NOT NULL,
    created_at INTEGER NOT NULL
);
CREATE INDEX page_revisions_page ON page_revisions(page_id, created_at DESC, id DESC);

CREATE TABLE slug_redirects (
    old_slug TEXT PRIMARY KEY,
    page_id INTEGER NOT NULL REFERENCES pages(id) ON DELETE CASCADE,
    created_at INTEGER NOT NULL
);

CREATE VIRTUAL TABLE pages_fts USING fts5(
    title,
    search_text,
    content='pages',
    content_rowid='id',
    tokenize='unicode61 remove_diacritics 2'
);

CREATE TRIGGER pages_ai AFTER INSERT ON pages BEGIN
  INSERT INTO pages_fts(rowid, title, search_text) VALUES (new.id, new.title, new.search_text);
END;
CREATE TRIGGER pages_ad AFTER DELETE ON pages BEGIN
  INSERT INTO pages_fts(pages_fts, rowid, title, search_text) VALUES ('delete', old.id, old.title, old.search_text);
END;
CREATE TRIGGER pages_au AFTER UPDATE ON pages BEGIN
  INSERT INTO pages_fts(pages_fts, rowid, title, search_text) VALUES ('delete', old.id, old.title, old.search_text);
  INSERT INTO pages_fts(rowid, title, search_text) VALUES (new.id, new.title, new.search_text);
END;
