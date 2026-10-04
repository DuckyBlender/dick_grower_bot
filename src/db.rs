use log::info;
use sqlx::{AssertSqlSafe, SqliteExecutor, SqlitePool};

/// Columns added after the initial release. Older databases may predate the migration that
/// introduced them, so they are added on startup when missing.
const LATE_COLUMNS: &[(&str, &str, &str)] = &[
    ("dicks", "daily_last_claimed", "TEXT DEFAULT NULL"),
    (
        "dicks",
        "daily_growth_boost_percent",
        "INTEGER NOT NULL DEFAULT 0",
    ),
    (
        "dicks",
        "daily_cooldown_skips",
        "INTEGER NOT NULL DEFAULT 0",
    ),
    ("dicks", "daily_streak_savers", "INTEGER NOT NULL DEFAULT 0"),
    ("dicks", "daily_lucky_rolls", "INTEGER NOT NULL DEFAULT 0"),
    ("dicks", "daily_streak", "INTEGER NOT NULL DEFAULT 0"),
    ("dicks", "best_daily_streak", "INTEGER NOT NULL DEFAULT 0"),
    ("dicks", "last_streak_date", "TEXT DEFAULT NULL"),
    ("dicks", "streak_last_claimed", "TEXT DEFAULT NULL"),
    ("global_events", "pot_amount", "INTEGER NOT NULL DEFAULT 0"),
    ("global_events", "resolved_at", "TEXT DEFAULT NULL"),
];

pub async fn ensure_schema(db: &SqlitePool) -> sqlx::Result<()> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS global_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            event_type TEXT NOT NULL,
            name TEXT NOT NULL,
            description TEXT NOT NULL,
            bonus_value INTEGER NOT NULL,
            pot_amount INTEGER NOT NULL DEFAULT 0,
            resolved_at TEXT DEFAULT NULL,
            started_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            ends_at TEXT NOT NULL
        )",
    )
    .execute(db)
    .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_global_events_ends_at ON global_events(ends_at)")
        .execute(db)
        .await?;

    for &(table, column, definition) in LATE_COLUMNS {
        // A missing table has no columns; it's left for the migrations to create.
        let (column_count, matching): (i64, i64) =
            sqlx::query_as("SELECT COUNT(*), COALESCE(SUM(name = ?), 0) FROM pragma_table_info(?)")
                .bind(column)
                .bind(table)
                .fetch_one(db)
                .await?;

        if column_count > 0 && matching == 0 {
            info!("Adding missing column {table}.{column}");
            // Identifiers can't be bound; these come from the constant table above.
            let statement = format!("ALTER TABLE {table} ADD COLUMN {column} {definition}");
            sqlx::query(AssertSqlSafe(statement)).execute(db).await?;
        }
    }

    Ok(())
}

/// Creates the user's row if it doesn't exist yet. New users start with a `last_grow` far in the
/// past so they can grow immediately but don't count as "active" for Dick of the Day.
pub async fn ensure_user(db: &SqlitePool, user_id: &str, guild_id: &str) -> sqlx::Result<()> {
    let result = sqlx::query!(
        "INSERT OR IGNORE INTO dicks (user_id, guild_id, last_grow)
         VALUES (?, ?, '1970-01-01 00:00:00')",
        user_id,
        guild_id
    )
    .execute(db)
    .await?;

    if result.rows_affected() > 0 {
        info!("Added user {user_id} in guild {guild_id} to the database");
    }
    Ok(())
}

pub async fn length(db: &SqlitePool, user_id: &str, guild_id: &str) -> sqlx::Result<i64> {
    let length = sqlx::query_scalar!(
        "SELECT length FROM dicks WHERE user_id = ? AND guild_id = ?",
        user_id,
        guild_id
    )
    .fetch_optional(db)
    .await?;
    Ok(length.unwrap_or_default())
}

/// 1-based leaderboard position of a given length within a guild.
pub async fn guild_rank(db: &SqlitePool, guild_id: &str, length: i64) -> sqlx::Result<usize> {
    let above = sqlx::query_scalar!(
        "SELECT COUNT(*) FROM dicks WHERE guild_id = ? AND length > ?",
        guild_id,
        length
    )
    .fetch_one(db)
    .await?;
    Ok(above as usize + 1)
}

#[derive(Clone, Copy)]
pub enum History {
    Grow,
    Streak,
    DailyBonus,
    Dotd,
    GiftSent,
    GiftReceived,
    PvpWon,
    PvpLost,
    CommunityPot,
}

impl History {
    pub fn as_str(self) -> &'static str {
        match self {
            History::Grow => "grow",
            History::Streak => "streak",
            History::DailyBonus => "daily_bonus",
            History::Dotd => "dotd",
            History::GiftSent => "gift_sent",
            History::GiftReceived => "gift_received",
            History::PvpWon => "pvp_won",
            History::PvpLost => "pvp_lost",
            History::CommunityPot => "community_pot",
        }
    }
}

pub async fn log_history(
    db: impl SqliteExecutor<'_>,
    user_id: &str,
    guild_id: &str,
    new_length: i64,
    change: i64,
    kind: History,
) -> sqlx::Result<()> {
    let kind = kind.as_str();
    sqlx::query!(
        "INSERT INTO length_history (user_id, guild_id, length, growth_amount, growth_type)
         VALUES (?, ?, ?, ?, ?)",
        user_id,
        guild_id,
        new_length,
        change,
        kind
    )
    .execute(db)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn database(migrations: &[&'static str]) -> SqlitePool {
        // A single connection, since every in-memory connection is its own database.
        let db = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        for migration in migrations {
            sqlx::raw_sql(*migration).execute(&db).await.unwrap();
        }
        db
    }

    const INITIAL: &str = include_str!("../migrations/20250309235354_initialize.sql");
    const FEATURES: &str = include_str!("../migrations/20250310000000_add_features.sql");

    #[tokio::test]
    async fn ensure_schema_upgrades_old_databases() {
        let db = database(&[INITIAL, FEATURES]).await;
        ensure_schema(&db).await.unwrap();
        ensure_schema(&db).await.unwrap();

        let streak: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pragma_table_info('dicks') WHERE name = 'daily_streak'",
        )
        .fetch_one(&db)
        .await
        .unwrap();
        assert_eq!(streak, 1);
    }

    #[tokio::test]
    async fn ensure_user_is_idempotent_and_starts_inactive() {
        let db = database(&[INITIAL, FEATURES]).await;
        ensure_schema(&db).await.unwrap();
        ensure_user(&db, "1", "2").await.unwrap();
        ensure_user(&db, "1", "2").await.unwrap();

        let (count, active): (i64, bool) = sqlx::query_as(
            "SELECT COUNT(*), MAX(last_grow > datetime('now', '-7 days')) FROM dicks",
        )
        .fetch_one(&db)
        .await
        .unwrap();
        assert_eq!(count, 1);
        assert!(!active);
        assert_eq!(length(&db, "1", "2").await.unwrap(), 0);
        assert_eq!(guild_rank(&db, "2", 0).await.unwrap(), 1);
    }
}
