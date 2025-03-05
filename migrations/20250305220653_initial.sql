-- Add migration script here
CREATE TABLE IF NOT EXISTS dicks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id TEXT NOT NULL,
    guild_id TEXT NOT NULL,
    length INTEGER NOT NULL DEFAULT 0,
    last_grow TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    dick_of_day_count INTEGER NOT NULL DEFAULT 0,
    UNIQUE(user_id, guild_id)
);
