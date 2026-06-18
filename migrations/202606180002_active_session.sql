CREATE TABLE wiki_settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    active_session_page_id INTEGER REFERENCES pages(id) ON DELETE SET NULL
);

INSERT INTO wiki_settings(id, active_session_page_id) VALUES (1, NULL);
