-- Add prestige functionality to dicks table
ALTER TABLE dicks ADD COLUMN prestige_level INTEGER NOT NULL DEFAULT 0;
ALTER TABLE dicks ADD COLUMN prestige_points INTEGER NOT NULL DEFAULT 0;

-- Create prestige_history table for tracking prestige events
CREATE TABLE IF NOT EXISTS prestige_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id TEXT NOT NULL,
    guild_id TEXT NOT NULL,
    prestige_level INTEGER NOT NULL,
    length_before_reset INTEGER NOT NULL,
    timestamp TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Add index for efficient querying
CREATE INDEX IF NOT EXISTS idx_prestige_history_user_guild ON prestige_history(user_id, guild_id);
CREATE INDEX IF NOT EXISTS idx_prestige_history_timestamp ON prestige_history(timestamp);
