use crate::Bot;
use crate::commands::escape_markdown;
use chrono::{DateTime, Datelike, Duration, NaiveDate, NaiveDateTime, Utc};
use log::{error, info};
use serenity::all::{
    CommandInteraction, CreateAllowedMentions, CreateEmbed, CreateEmbedFooter,
    CreateInteractionResponse, CreateInteractionResponseMessage, GuildId, UserId,
};
use serenity::prelude::*;
use sqlx::{Pool, Row, Sqlite, Transaction};

const TIMESTAMP_FORMAT: &str = "%Y-%m-%d %H:%M:%S";

#[derive(Debug, Clone)]
pub struct Season {
    pub id: i64,
    pub name: String,
    pub ends_at: NaiveDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileSeasonStats {
    pub score: i64,
    pub rank: i64,
    pub gap_to_next: Option<i64>,
}

fn parse_timestamp(value: &str) -> Result<NaiveDateTime, sqlx::Error> {
    NaiveDateTime::parse_from_str(value, TIMESTAMP_FORMAT)
        .map_err(|why| sqlx::Error::Decode(Box::new(why)))
}

fn format_timestamp(value: NaiveDateTime) -> String {
    value.format(TIMESTAMP_FORMAT).to_string()
}

fn first_of_next_month(now: NaiveDateTime) -> NaiveDateTime {
    let (year, month) = if now.month() == 12 {
        (now.year() + 1, 1)
    } else {
        (now.year(), now.month() + 1)
    };
    NaiveDate::from_ymd_opt(year, month, 1)
        .expect("valid first day of month")
        .and_hms_opt(0, 0, 0)
        .expect("valid midnight")
}

fn first_of_month(now: NaiveDateTime) -> NaiveDateTime {
    NaiveDate::from_ymd_opt(now.year(), now.month(), 1)
        .expect("valid first day of month")
        .and_hms_opt(0, 0, 0)
        .expect("valid midnight")
}

fn month_name(month: u32) -> &'static str {
    match month {
        1 => "January",
        2 => "February",
        3 => "March",
        4 => "April",
        5 => "May",
        6 => "June",
        7 => "July",
        8 => "August",
        9 => "September",
        10 => "October",
        11 => "November",
        12 => "December",
        _ => "Unknown",
    }
}

fn inaugural_end(now: NaiveDateTime) -> NaiveDateTime {
    let next_boundary = first_of_next_month(now);
    if next_boundary - now < Duration::days(14) {
        first_of_next_month(next_boundary)
    } else {
        next_boundary
    }
}

async fn finalize_ended_seasons(
    database: &Pool<Sqlite>,
    now: NaiveDateTime,
) -> Result<(), sqlx::Error> {
    let now_string = format_timestamp(now);
    let ended = sqlx::query(
        "SELECT id FROM seasons
         WHERE finalized_at IS NULL AND ends_at <= ?
         ORDER BY ends_at ASC",
    )
    .bind(&now_string)
    .fetch_all(database)
    .await?;

    for row in ended {
        let season_id: i64 = row.try_get("id")?;
        let mut tx = database.begin().await?;

        sqlx::query(
            "INSERT OR IGNORE INTO season_placements
                (season_id, scope, guild_id, profile_guild_id, user_id, position, score)
             SELECT season_id, 'global', '', guild_id, user_id, position, score
             FROM (
                 SELECT season_id, guild_id, user_id, score,
                        ROW_NUMBER() OVER (
                            ORDER BY score DESC, user_id ASC, guild_id ASC
                        ) AS position
                 FROM season_scores
                 WHERE season_id = ?
             )
             WHERE position <= 3",
        )
        .bind(season_id)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            "INSERT OR IGNORE INTO season_placements
                (season_id, scope, guild_id, profile_guild_id, user_id, position, score)
             SELECT season_id, 'server', guild_id, guild_id, user_id, position, score
             FROM (
                 SELECT season_id, guild_id, user_id, score,
                        ROW_NUMBER() OVER (
                            PARTITION BY guild_id
                            ORDER BY score DESC, user_id ASC
                        ) AS position
                 FROM season_scores
                 WHERE season_id = ?
             )
             WHERE position <= 3",
        )
        .bind(season_id)
        .execute(&mut *tx)
        .await?;

        sqlx::query("UPDATE seasons SET finalized_at = ? WHERE id = ? AND finalized_at IS NULL")
            .bind(&now_string)
            .bind(season_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        info!("Finalized season {}", season_id);
    }

    Ok(())
}

pub async fn ensure_active_season(
    database: &Pool<Sqlite>,
    now: NaiveDateTime,
) -> Result<Season, sqlx::Error> {
    finalize_ended_seasons(database, now).await?;
    let now_string = format_timestamp(now);

    if let Some(row) = sqlx::query(
        "SELECT id, season_number, name, starts_at, ends_at
         FROM seasons
         WHERE starts_at <= ? AND ends_at > ?
         ORDER BY season_number DESC
         LIMIT 1",
    )
    .bind(&now_string)
    .bind(&now_string)
    .fetch_optional(database)
    .await?
    {
        return Ok(Season {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            ends_at: parse_timestamp(&row.try_get::<String, _>("ends_at")?)?,
        });
    }

    let last = sqlx::query("SELECT MAX(season_number) AS number FROM seasons")
        .fetch_one(database)
        .await?;
    let last_number = last.try_get::<Option<i64>, _>("number")?.unwrap_or(0);
    let season_number = last_number + 1;
    let (starts_at, ends_at) = if last_number == 0 {
        (now, inaugural_end(now))
    } else {
        (first_of_month(now), first_of_next_month(now))
    };
    let name = format!(
        "Season {} • {} {}",
        season_number,
        month_name(starts_at.month()),
        starts_at.year()
    );

    sqlx::query(
        "INSERT OR IGNORE INTO seasons (season_number, name, starts_at, ends_at)
         VALUES (?, ?, ?, ?)",
    )
    .bind(season_number)
    .bind(&name)
    .bind(format_timestamp(starts_at))
    .bind(format_timestamp(ends_at))
    .execute(database)
    .await?;
    let row = sqlx::query(
        "SELECT id, name, ends_at FROM seasons
         WHERE starts_at <= ? AND ends_at > ?
         ORDER BY season_number DESC LIMIT 1",
    )
    .bind(&now_string)
    .bind(&now_string)
    .fetch_one(database)
    .await?;
    Ok(Season {
        id: row.try_get("id")?,
        name: row.try_get("name")?,
        ends_at: parse_timestamp(&row.try_get::<String, _>("ends_at")?)?,
    })
}

pub async fn active_season(database: &Pool<Sqlite>) -> Result<Season, sqlx::Error> {
    ensure_active_season(database, Utc::now().naive_utc()).await
}

pub async fn credit_earned_growth(
    tx: &mut Transaction<'_, Sqlite>,
    season_id: i64,
    user_id: &str,
    guild_id: &str,
    amount: i64,
) -> Result<(), sqlx::Error> {
    if amount <= 0 {
        return Ok(());
    }

    let profile_update = sqlx::query(
        "UPDATE dicks
         SET prestige_progress = prestige_progress + ?
         WHERE user_id = ? AND guild_id = ?",
    )
    .bind(amount)
    .bind(user_id)
    .bind(guild_id)
    .execute(&mut **tx)
    .await?;
    if profile_update.rows_affected() != 1 {
        return Err(sqlx::Error::RowNotFound);
    }

    sqlx::query(
        "INSERT INTO season_scores (season_id, user_id, guild_id, score, updated_at)
         VALUES (?, ?, ?, ?, CURRENT_TIMESTAMP)
         ON CONFLICT(season_id, user_id, guild_id)
         DO UPDATE SET score = score + excluded.score, updated_at = CURRENT_TIMESTAMP",
    )
    .bind(season_id)
    .bind(user_id)
    .bind(guild_id)
    .bind(amount)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

async fn selected_season(
    database: &Pool<Sqlite>,
    period: &str,
) -> Result<Option<Season>, sqlx::Error> {
    let current = active_season(database).await?;
    if period != "previous" {
        return Ok(Some(current));
    }

    let row = sqlx::query(
        "SELECT id, season_number, name, starts_at, ends_at
         FROM seasons
         WHERE finalized_at IS NOT NULL AND id <> ?
         ORDER BY ends_at DESC
         LIMIT 1",
    )
    .bind(current.id)
    .fetch_optional(database)
    .await?;

    row.map(|row| {
        Ok(Season {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            ends_at: parse_timestamp(&row.try_get::<String, _>("ends_at")?)?,
        })
    })
    .transpose()
}

pub fn cached_user_label(ctx: &Context, user_id: &str) -> String {
    let Ok(id) = user_id.parse::<u64>() else {
        return "Unknown User".to_string();
    };
    ctx.cache
        .user(UserId::new(id))
        .map(|user| escape_markdown(&user.name))
        .unwrap_or_else(|| format!("<@{id}>"))
}

pub fn cached_guild_label(ctx: &Context, guild_id: &str) -> String {
    let Ok(id) = guild_id.parse::<u64>() else {
        return "unknown server".to_string();
    };
    ctx.cache
        .guild(GuildId::new(id))
        .map(|guild| {
            if guild.features.iter().any(|feature| feature == "COMMUNITY") {
                escape_markdown(&guild.name)
            } else {
                "private server".to_string()
            }
        })
        .unwrap_or_else(|| "unknown server".to_string())
}

pub async fn get_profile_season_stats(
    database: &Pool<Sqlite>,
    season_id: i64,
    user_id: &str,
    guild_id: &str,
    global: bool,
) -> Result<Option<ProfileSeasonStats>, sqlx::Error> {
    let score = sqlx::query(
        "SELECT score FROM season_scores
         WHERE season_id = ? AND user_id = ? AND guild_id = ?",
    )
    .bind(season_id)
    .bind(user_id)
    .bind(guild_id)
    .fetch_optional(database)
    .await?;
    let Some(score) = score else {
        return Ok(None);
    };
    let score: i64 = score.try_get("score")?;

    let (rank, next_score) = if global {
        let rank = sqlx::query(
            "SELECT COUNT(*) + 1 AS rank FROM season_scores
             WHERE season_id = ? AND (
                 score > ? OR
                 (score = ? AND user_id < ?) OR
                 (score = ? AND user_id = ? AND guild_id < ?)
             )",
        )
        .bind(season_id)
        .bind(score)
        .bind(score)
        .bind(user_id)
        .bind(score)
        .bind(user_id)
        .bind(guild_id)
        .fetch_one(database)
        .await?
        .try_get::<i64, _>("rank")?;
        let next = sqlx::query(
            "SELECT score FROM season_scores
             WHERE season_id = ? AND (
                 score > ? OR
                 (score = ? AND user_id < ?) OR
                 (score = ? AND user_id = ? AND guild_id < ?)
             )
             ORDER BY score ASC, user_id DESC, guild_id DESC
             LIMIT 1",
        )
        .bind(season_id)
        .bind(score)
        .bind(score)
        .bind(user_id)
        .bind(score)
        .bind(user_id)
        .bind(guild_id)
        .fetch_optional(database)
        .await?
        .map(|row| row.try_get::<i64, _>("score"))
        .transpose()?;
        (rank, next)
    } else {
        let rank = sqlx::query(
            "SELECT COUNT(*) + 1 AS rank FROM season_scores
             WHERE season_id = ? AND guild_id = ? AND (
                 score > ? OR (score = ? AND user_id < ?)
             )",
        )
        .bind(season_id)
        .bind(guild_id)
        .bind(score)
        .bind(score)
        .bind(user_id)
        .fetch_one(database)
        .await?
        .try_get::<i64, _>("rank")?;
        let next = sqlx::query(
            "SELECT score FROM season_scores
             WHERE season_id = ? AND guild_id = ? AND (
                 score > ? OR (score = ? AND user_id < ?)
             )
             ORDER BY score ASC, user_id DESC
             LIMIT 1",
        )
        .bind(season_id)
        .bind(guild_id)
        .bind(score)
        .bind(score)
        .bind(user_id)
        .fetch_optional(database)
        .await?
        .map(|row| row.try_get::<i64, _>("score"))
        .transpose()?;
        (rank, next)
    };

    Ok(Some(ProfileSeasonStats {
        score,
        rank,
        gap_to_next: next_score.map(|next| next - score),
    }))
}

pub async fn recent_medals(
    database: &Pool<Sqlite>,
    user_id: &str,
    guild_id: &str,
) -> Result<Vec<String>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT p.scope, p.position, s.name
         FROM season_placements p
         JOIN seasons s ON s.id = p.season_id
         WHERE p.user_id = ? AND p.profile_guild_id = ?
           AND p.season_id = (SELECT MAX(id) FROM seasons WHERE finalized_at IS NOT NULL)
         ORDER BY p.scope ASC",
    )
    .bind(user_id)
    .bind(guild_id)
    .fetch_all(database)
    .await?;

    rows.into_iter()
        .map(|row| {
            let scope: String = row.try_get("scope")?;
            let position: i64 = row.try_get("position")?;
            let name: String = row.try_get("name")?;
            let medal = match position {
                1 => "🥇",
                2 => "🥈",
                _ => "🥉",
            };
            Ok(format!("{medal} {name} {scope} #{position}"))
        })
        .collect()
}

pub async fn handle_season_command(
    ctx: &Context,
    command: &CommandInteraction,
) -> Result<(), serenity::Error> {
    let data = ctx.data.read().await;
    let bot = data.get::<Bot>().unwrap();
    let scope = command
        .data
        .options
        .iter()
        .find(|option| option.name == "scope")
        .and_then(|option| option.value.as_str())
        .unwrap_or("server");
    let period = command
        .data
        .options
        .iter()
        .find(|option| option.name == "period")
        .and_then(|option| option.value.as_str())
        .unwrap_or("current");
    let global = scope == "global";
    let guild_id = command.guild_id.expect("guild command").to_string();

    let season = match selected_season(&bot.database, period).await {
        Ok(Some(season)) => season,
        Ok(None) => {
            return command
                .create_response(
                    &ctx.http,
                    CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new()
                            .content("There is no completed season yet.")
                            .ephemeral(true),
                    ),
                )
                .await;
        }
        Err(why) => {
            error!("Failed to select season: {:?}", why);
            return command
                .create_response(
                    &ctx.http,
                    CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new()
                            .content("The season leaderboard is unavailable right now.")
                            .ephemeral(true),
                    ),
                )
                .await;
        }
    };

    let rows = if global {
        sqlx::query(
            "SELECT user_id, guild_id, score FROM season_scores
             WHERE season_id = ?
             ORDER BY score DESC, user_id ASC, guild_id ASC
             LIMIT 10",
        )
        .bind(season.id)
        .fetch_all(&bot.database)
        .await
    } else {
        sqlx::query(
            "SELECT user_id, guild_id, score FROM season_scores
             WHERE season_id = ? AND guild_id = ?
             ORDER BY score DESC, user_id ASC
             LIMIT 10",
        )
        .bind(season.id)
        .bind(&guild_id)
        .fetch_all(&bot.database)
        .await
    };

    let rows = match rows {
        Ok(rows) => rows,
        Err(why) => {
            error!("Failed to query season leaderboard: {:?}", why);
            return command
                .create_response(
                    &ctx.http,
                    CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new()
                            .content("The season leaderboard is unavailable right now."),
                    ),
                )
                .await;
        }
    };

    let mut description = if rows.is_empty() {
        "No one has earned seasonal growth here yet. Use `/grow` to get started!".to_string()
    } else {
        String::new()
    };
    for (index, row) in rows.iter().enumerate() {
        let user_id: String = row.try_get("user_id").unwrap_or_default();
        let profile_guild: String = row.try_get("guild_id").unwrap_or_default();
        let score: i64 = row.try_get("score").unwrap_or_default();
        let marker = match index {
            0 => "🥇",
            1 => "🥈",
            2 => "🥉",
            _ => "🔹",
        };
        let server = if global {
            format!(" ({})", cached_guild_label(ctx, &profile_guild))
        } else {
            String::new()
        };
        description.push_str(&format!(
            "{marker} **{}. {}**: **{} cm**{server}\n",
            index + 1,
            cached_user_label(ctx, &user_id),
            score
        ));
    }

    let profile = get_profile_season_stats(
        &bot.database,
        season.id,
        &command.user.id.to_string(),
        &guild_id,
        global,
    )
    .await
    .ok()
    .flatten();
    if let Some(profile) = profile {
        let gap = profile
            .gap_to_next
            .map(|gap| format!(" • {} cm to the next score", gap))
            .unwrap_or_else(|| " • leading this board".to_string());
        description.push_str(&format!(
            "\n**Your profile:** #{} with {} cm{}",
            profile.rank, profile.score, gap
        ));
    } else {
        description.push_str("\n**Your profile:** unranked this season");
    }

    let ends_unix = DateTime::<Utc>::from_naive_utc_and_offset(season.ends_at, Utc).timestamp();
    let period_label = if period == "previous" {
        "Previous"
    } else {
        "Current"
    };
    let scope_label = if global { "Global" } else { "Server" };
    let footer = if period == "previous" {
        format!(
            "{} • Final standings • Gifts, PvP, and prestige resets do not count",
            season.name
        )
    } else {
        format!(
            "{} • Ends <t:{}:R> • Gifts, PvP, and prestige resets do not count",
            season.name, ends_unix
        )
    };
    command
        .create_response(
            &ctx.http,
            CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .allowed_mentions(CreateAllowedMentions::new())
                    .add_embed(
                        CreateEmbed::new()
                            .title(format!("🏁 {period_label} {scope_label} Season"))
                            .description(description)
                            .color(0xE67E22)
                            .footer(CreateEmbedFooter::new(footer)),
                    ),
            ),
        )
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    fn timestamp(year: i32, month: u32, day: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(year, month, day)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap()
    }

    async fn test_database() -> Pool<Sqlite> {
        let database = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        for statement in [
            "CREATE TABLE dicks (
                user_id TEXT NOT NULL, guild_id TEXT NOT NULL,
                length INTEGER NOT NULL DEFAULT 0,
                prestige_progress INTEGER NOT NULL DEFAULT 0,
                UNIQUE(user_id, guild_id)
            )",
            "CREATE TABLE seasons (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                season_number INTEGER NOT NULL UNIQUE,
                name TEXT NOT NULL, starts_at TEXT NOT NULL, ends_at TEXT NOT NULL,
                finalized_at TEXT DEFAULT NULL
            )",
            "CREATE TABLE season_scores (
                season_id INTEGER NOT NULL, user_id TEXT NOT NULL, guild_id TEXT NOT NULL,
                score INTEGER NOT NULL DEFAULT 0, updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                PRIMARY KEY(season_id, user_id, guild_id)
            )",
            "CREATE TABLE season_placements (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                season_id INTEGER NOT NULL, scope TEXT NOT NULL, guild_id TEXT NOT NULL DEFAULT '',
                profile_guild_id TEXT NOT NULL, user_id TEXT NOT NULL,
                position INTEGER NOT NULL, score INTEGER NOT NULL,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                UNIQUE(season_id, scope, guild_id, position)
            )",
            "CREATE INDEX idx_dicks_global_leaderboard ON dicks(length DESC, user_id ASC, guild_id ASC)",
            "CREATE INDEX idx_dicks_guild_leaderboard ON dicks(guild_id, length DESC, user_id ASC)",
            "CREATE INDEX idx_season_scores_global ON season_scores(season_id, score DESC, user_id ASC, guild_id ASC)",
            "CREATE INDEX idx_season_scores_guild ON season_scores(season_id, guild_id, score DESC, user_id ASC)",
        ] {
            sqlx::query(statement).execute(&database).await.unwrap();
        }
        database
    }

    #[test]
    fn inaugural_season_extends_when_less_than_fourteen_days_remain() {
        assert_eq!(inaugural_end(timestamp(2026, 7, 26)), timestamp(2026, 9, 1));
        assert_eq!(inaugural_end(timestamp(2026, 7, 10)), timestamp(2026, 8, 1));
    }

    #[test]
    fn month_boundaries_handle_years_and_leap_years() {
        assert_eq!(
            first_of_next_month(timestamp(2026, 12, 20)),
            timestamp(2027, 1, 1)
        );
        assert_eq!(
            first_of_next_month(timestamp(2028, 2, 29)),
            timestamp(2028, 3, 1)
        );
    }

    #[tokio::test]
    async fn earned_growth_updates_progress_and_score_but_transfers_do_not() {
        let database = test_database().await;
        sqlx::query("INSERT INTO dicks (user_id, guild_id) VALUES ('u1', 'g1')")
            .execute(&database)
            .await
            .unwrap();
        let season = ensure_active_season(&database, timestamp(2026, 7, 10))
            .await
            .unwrap();
        let mut tx = database.begin().await.unwrap();
        sqlx::query(
            "UPDATE dicks SET length = length + 10 WHERE user_id = 'u1' AND guild_id = 'g1'",
        )
        .execute(&mut *tx)
        .await
        .unwrap();
        credit_earned_growth(&mut tx, season.id, "u1", "g1", 10)
            .await
            .unwrap();
        tx.commit().await.unwrap();

        // A gift changes length only and deliberately bypasses seasonal/prestige crediting.
        sqlx::query(
            "UPDATE dicks SET length = length + 5 WHERE user_id = 'u1' AND guild_id = 'g1'",
        )
        .execute(&database)
        .await
        .unwrap();
        let profile = sqlx::query(
            "SELECT length, prestige_progress FROM dicks WHERE user_id = 'u1' AND guild_id = 'g1'",
        )
        .fetch_one(&database)
        .await
        .unwrap();
        assert_eq!(profile.try_get::<i64, _>("length").unwrap(), 15);
        assert_eq!(profile.try_get::<i64, _>("prestige_progress").unwrap(), 10);
        let score = sqlx::query_scalar::<_, i64>(
            "SELECT score FROM season_scores WHERE season_id = ? AND user_id = 'u1'",
        )
        .bind(season.id)
        .fetch_one(&database)
        .await
        .unwrap();
        assert_eq!(score, 10);
        sqlx::query(
            "INSERT INTO season_scores (season_id, user_id, guild_id, score)
             VALUES (?, 'a', 'g1', 10)",
        )
        .bind(season.id)
        .execute(&database)
        .await
        .unwrap();
        let stats = get_profile_season_stats(&database, season.id, "u1", "g1", false)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stats.rank, 2);
        assert_eq!(stats.gap_to_next, Some(0));
    }

    #[tokio::test]
    async fn rollover_finalizes_deterministic_global_and_server_medals() {
        let database = test_database().await;
        let season = ensure_active_season(&database, timestamp(2026, 7, 10))
            .await
            .unwrap();
        for (user, guild, score) in [("b", "g1", 100), ("a", "g1", 100), ("c", "g2", 90)] {
            sqlx::query(
                "INSERT INTO season_scores (season_id, user_id, guild_id, score) VALUES (?, ?, ?, ?)",
            )
            .bind(season.id)
            .bind(user)
            .bind(guild)
            .bind(score)
            .execute(&database)
            .await
            .unwrap();
        }
        ensure_active_season(&database, timestamp(2026, 8, 1))
            .await
            .unwrap();
        let global_winner = sqlx::query_scalar::<_, String>(
            "SELECT user_id FROM season_placements
             WHERE season_id = ? AND scope = 'global' AND position = 1",
        )
        .bind(season.id)
        .fetch_one(&database)
        .await
        .unwrap();
        assert_eq!(global_winner, "a");
        let server_winner = sqlx::query_scalar::<_, String>(
            "SELECT user_id FROM season_placements
             WHERE season_id = ? AND scope = 'server' AND guild_id = 'g1' AND position = 1",
        )
        .bind(season.id)
        .fetch_one(&database)
        .await
        .unwrap();
        assert_eq!(server_winner, "a");
    }

    #[tokio::test]
    async fn leaderboard_queries_use_covering_indexes() {
        let database = test_database().await;
        let global_plan = sqlx::query(
            "EXPLAIN QUERY PLAN SELECT user_id, length, guild_id FROM dicks
             ORDER BY length DESC, user_id ASC, guild_id ASC LIMIT 10",
        )
        .fetch_all(&database)
        .await
        .unwrap();
        let global_detail = global_plan
            .iter()
            .filter_map(|row| row.try_get::<String, _>("detail").ok())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(global_detail.contains("idx_dicks_global_leaderboard"));

        let season_plan = sqlx::query(
            "EXPLAIN QUERY PLAN SELECT user_id, guild_id, score FROM season_scores
             WHERE season_id = 1 ORDER BY score DESC, user_id ASC, guild_id ASC LIMIT 10",
        )
        .fetch_all(&database)
        .await
        .unwrap();
        let season_detail = season_plan
            .iter()
            .filter_map(|row| row.try_get::<String, _>("detail").ok())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(season_detail.contains("idx_season_scores_global"));
    }
}
