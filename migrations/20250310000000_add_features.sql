-- Add viagra functionality to dicks table
ALTER TABLE dicks ADD COLUMN viagra_active_until TEXT DEFAULT NULL;
ALTER TABLE dicks ADD COLUMN viagra_last_used TEXT DEFAULT NULL;

-- Create length_history table for tracking user growth over time
CREATE TABLE IF NOT EXISTS length_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id TEXT NOT NULL,
    guild_id TEXT NOT NULL,
    length INTEGER NOT NULL,
    growth_amount INTEGER NOT NULL,
    timestamp TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    growth_type TEXT NOT NULL DEFAULT 'grow' -- 'grow', 'gift_sent', 'gift_received', 'pvp_won', 'pvp_lost', 'dotd'
);

-- Add index for efficient querying
CREATE INDEX IF NOT EXISTS idx_length_history_user_guild ON length_history(user_id, guild_id);
CREATE INDEX IF NOT EXISTS idx_length_history_timestamp ON length_history(timestamp);

-- Add guild_name cache to guild_settings for optimization
ALTER TABLE guild_settings ADD COLUMN guild_name TEXT DEFAULT NULL;
ALTER TABLE guild_settings ADD COLUMN guild_name_cached_at TEXT DEFAULT NULL; 