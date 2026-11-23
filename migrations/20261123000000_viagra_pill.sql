-- /viagra now hands out one pill per UTC day that waits for the player's next /grow.
-- viagra_last_used records when the pill was taken; viagra_active_until is no longer used.
ALTER TABLE dicks ADD COLUMN viagra_pill_pending INTEGER NOT NULL DEFAULT 0;
