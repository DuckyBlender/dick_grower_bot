-- postgresql_schema.sql

CREATE TABLE IF NOT EXISTS dicks (
    id SERIAL PRIMARY KEY,
    user_id TEXT NOT NULL,
    guild_id TEXT NOT NULL,
    length INTEGER NOT NULL DEFAULT 0, -- Changed from INTEGER
    last_grow TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    growth_count INTEGER NOT NULL DEFAULT 0, -- Changed from INTEGER
    dick_of_day_count INTEGER NOT NULL DEFAULT 0, -- Changed from INTEGER
    pvp_wins INTEGER NOT NULL DEFAULT 0, -- Changed from INTEGER
    pvp_losses INTEGER NOT NULL DEFAULT 0, -- Changed from INTEGER
    pvp_max_streak INTEGER NOT NULL DEFAULT 0, -- Changed from INTEGER
    pvp_current_streak INTEGER NOT NULL DEFAULT 0, -- Changed from INTEGER
    cm_won INTEGER NOT NULL DEFAULT 0, -- Changed from INTEGER
    cm_lost INTEGER NOT NULL DEFAULT 0, -- Changed from INTEGER
    UNIQUE(user_id, guild_id)
);

CREATE TABLE IF NOT EXISTS guild_settings (
    id SERIAL PRIMARY KEY,
    guild_id TEXT NOT NULL UNIQUE,
    last_dotd TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);