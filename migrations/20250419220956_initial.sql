-- postgresql_schema.sql

CREATE TABLE IF NOT EXISTS dicks (
    id SERIAL PRIMARY KEY,
    user_id TEXT NOT NULL,
    guild_id TEXT NOT NULL,
    length BIGINT NOT NULL DEFAULT 0, -- Changed from INTEGER
    last_grow TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    growth_count BIGINT NOT NULL DEFAULT 0, -- Changed from INTEGER
    dick_of_day_count BIGINT NOT NULL DEFAULT 0, -- Changed from INTEGER
    pvp_wins BIGINT NOT NULL DEFAULT 0, -- Changed from INTEGER
    pvp_losses BIGINT NOT NULL DEFAULT 0, -- Changed from INTEGER
    pvp_max_streak BIGINT NOT NULL DEFAULT 0, -- Changed from INTEGER
    pvp_current_streak BIGINT NOT NULL DEFAULT 0, -- Changed from INTEGER
    cm_won BIGINT NOT NULL DEFAULT 0, -- Changed from INTEGER
    cm_lost BIGINT NOT NULL DEFAULT 0, -- Changed from INTEGER
    UNIQUE(user_id, guild_id)
);

CREATE TABLE IF NOT EXISTS guild_settings (
    id SERIAL PRIMARY KEY,
    guild_id TEXT NOT NULL UNIQUE,
    last_dotd TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);