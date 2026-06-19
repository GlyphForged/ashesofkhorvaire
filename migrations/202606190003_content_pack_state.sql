CREATE TABLE content_pack_state (
    pack TEXT NOT NULL,
    slug TEXT NOT NULL,
    applied_hash TEXT NOT NULL,
    applied_at INTEGER NOT NULL,
    PRIMARY KEY (pack, slug)
);

