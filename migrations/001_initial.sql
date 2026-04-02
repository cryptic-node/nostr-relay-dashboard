CREATE TABLE IF NOT EXISTS events (
    id          TEXT PRIMARY KEY,
    pubkey      TEXT NOT NULL,
    created_at  INTEGER NOT NULL,
    kind        INTEGER NOT NULL,
    tags        TEXT NOT NULL,
    content     TEXT NOT NULL,
    sig         TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS events_pubkey ON events(pubkey);
CREATE INDEX IF NOT EXISTS events_kind ON events(kind);
CREATE INDEX IF NOT EXISTS events_created_at ON events(created_at);

CREATE TABLE IF NOT EXISTS whitelist (
    pubkey      TEXT PRIMARY KEY,
    note        TEXT,
    added_at    INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);
