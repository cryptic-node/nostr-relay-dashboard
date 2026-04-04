-- Migration: Add upstream relays and monitored npubs tables
-- Run this after your initial migration

CREATE TABLE IF NOT EXISTS upstream_relays (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    url TEXT NOT NULL UNIQUE,
    name TEXT,
    enabled BOOLEAN DEFAULT 1,
    preloaded BOOLEAN DEFAULT 0,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS monitored_npubs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    npub TEXT NOT NULL UNIQUE,
    label TEXT,
    last_synced DATETIME,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Preload the top reliable free public relays (as of 2026)
INSERT OR IGNORE INTO upstream_relays (url, name, enabled, preloaded) VALUES
('wss://relay.damus.io', 'Damus', 1, 1),
('wss://nos.lol', 'nos.lol', 1, 1),
('wss://nostr.wine', 'Nostr Wine', 1, 1),
('wss://relay.snort.social', 'Snort', 1, 1),
('wss://nostr.mutinywallet.com', 'Mutiny', 1, 1);
