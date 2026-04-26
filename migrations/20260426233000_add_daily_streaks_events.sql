-- Daily rewards, consecutive growth streaks, and global events.
ALTER TABLE dicks ADD COLUMN daily_last_claimed TEXT DEFAULT NULL;
ALTER TABLE dicks ADD COLUMN daily_growth_boost_percent INTEGER NOT NULL DEFAULT 0;
ALTER TABLE dicks ADD COLUMN daily_cooldown_skips INTEGER NOT NULL DEFAULT 0;
ALTER TABLE dicks ADD COLUMN daily_streak_savers INTEGER NOT NULL DEFAULT 0;
ALTER TABLE dicks ADD COLUMN daily_lucky_rolls INTEGER NOT NULL DEFAULT 0;
ALTER TABLE dicks ADD COLUMN daily_streak INTEGER NOT NULL DEFAULT 0;
ALTER TABLE dicks ADD COLUMN best_daily_streak INTEGER NOT NULL DEFAULT 0;
ALTER TABLE dicks ADD COLUMN last_streak_date TEXT DEFAULT NULL;
ALTER TABLE dicks ADD COLUMN streak_last_claimed TEXT DEFAULT NULL;

CREATE TABLE IF NOT EXISTS global_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    event_type TEXT NOT NULL,
    name TEXT NOT NULL,
    description TEXT NOT NULL,
    bonus_value INTEGER NOT NULL,
    pot_amount INTEGER NOT NULL DEFAULT 0,
    resolved_at TEXT DEFAULT NULL,
    started_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    ends_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_global_events_ends_at ON global_events(ends_at);
