-- Seasonal leaderboards, prestige progression, and leaderboard indexes.
ALTER TABLE dicks ADD COLUMN prestige_level INTEGER NOT NULL DEFAULT 0;
ALTER TABLE dicks ADD COLUMN prestige_points INTEGER NOT NULL DEFAULT 0;
ALTER TABLE dicks ADD COLUMN prestige_progress INTEGER NOT NULL DEFAULT 0;

-- Existing length is treated as legacy earned progress exactly once at rollout.
UPDATE dicks
SET prestige_progress = CASE WHEN length > 0 THEN length ELSE 0 END;

CREATE TABLE IF NOT EXISTS seasons (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    season_number INTEGER NOT NULL UNIQUE,
    name TEXT NOT NULL,
    starts_at TEXT NOT NULL,
    ends_at TEXT NOT NULL,
    finalized_at TEXT DEFAULT NULL
);

CREATE TABLE IF NOT EXISTS season_scores (
    season_id INTEGER NOT NULL,
    user_id TEXT NOT NULL,
    guild_id TEXT NOT NULL,
    score INTEGER NOT NULL DEFAULT 0,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (season_id, user_id, guild_id),
    FOREIGN KEY (season_id) REFERENCES seasons(id)
);

CREATE TABLE IF NOT EXISTS season_placements (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    season_id INTEGER NOT NULL,
    scope TEXT NOT NULL CHECK (scope IN ('server', 'global')),
    guild_id TEXT NOT NULL DEFAULT '',
    profile_guild_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    position INTEGER NOT NULL CHECK (position BETWEEN 1 AND 3),
    score INTEGER NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (season_id, scope, guild_id, position),
    FOREIGN KEY (season_id) REFERENCES seasons(id)
);

CREATE TABLE IF NOT EXISTS prestige_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id TEXT NOT NULL,
    guild_id TEXT NOT NULL,
    prestige_level INTEGER NOT NULL,
    points_earned INTEGER NOT NULL,
    length_before_reset INTEGER NOT NULL,
    progress_before_reset INTEGER NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_dicks_global_leaderboard
    ON dicks(length DESC, user_id ASC, guild_id ASC);
CREATE INDEX IF NOT EXISTS idx_dicks_guild_leaderboard
    ON dicks(guild_id, length DESC, user_id ASC);
CREATE INDEX IF NOT EXISTS idx_season_scores_global
    ON season_scores(season_id, score DESC, user_id ASC, guild_id ASC);
CREATE INDEX IF NOT EXISTS idx_season_scores_guild
    ON season_scores(season_id, guild_id, score DESC, user_id ASC);
CREATE INDEX IF NOT EXISTS idx_season_placements_profile
    ON season_placements(user_id, profile_guild_id, season_id DESC);
CREATE INDEX IF NOT EXISTS idx_prestige_history_profile
    ON prestige_history(user_id, guild_id, created_at DESC);
