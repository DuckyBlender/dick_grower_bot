use crate::commands::global::guild_label;
use crate::commands::top::display_names;
use crate::commands::{Cmd, CommandResult};
use crate::time;
use crate::utils::{colors, embed, medal, ordinal};
use chrono::{Datelike, Months, NaiveDate, NaiveDateTime, NaiveTime};
use serenity::all::{
    CommandOptionType, CreateCommand, CreateCommandOption, CreateEmbedFooter, ResolvedValue,
};
use sqlx::{AssertSqlSafe, SqlitePool};

/// A season is a UTC calendar month. Standings are read straight from `length_history`:
/// only centimeters a player grew themselves count, so gifts and battles can't buy a
/// placement and nothing has to be reset when a season ends.
const STANDINGS: &str = "SELECT user_id, guild_id, SUM(growth_amount) AS score
     FROM length_history
     WHERE timestamp >= ?1 AND timestamp < ?2 AND (?3 IS NULL OR guild_id = ?3)
       AND growth_type IN ('grow', 'streak', 'daily_bonus', 'dotd', 'community_pot')
     GROUP BY user_id, guild_id";

pub fn register() -> CreateCommand {
    CreateCommand::new("season")
        .description("Show this month's leaderboard, counting only what was grown this month")
        .add_option(
            CreateCommandOption::new(CommandOptionType::String, "scope", "Which players to rank")
                .add_string_choice("This server", "server")
                .add_string_choice("Global", "global"),
        )
        .add_option(
            CreateCommandOption::new(CommandOptionType::String, "period", "Which season to show")
                .add_string_choice("Current", "current")
                .add_string_choice("Previous", "previous"),
        )
}

/// Start and end of the season containing `now`, or of the one before it.
fn bounds(now: NaiveDateTime, previous: bool) -> (NaiveDateTime, NaiveDateTime) {
    let first = NaiveDate::from_ymd_opt(now.year(), now.month(), 1)
        .expect("the first of a month always exists");
    let start = if previous {
        first - Months::new(1)
    } else {
        first
    };
    (
        start.and_time(NaiveTime::MIN),
        (start + Months::new(1)).and_time(NaiveTime::MIN),
    )
}

/// Top ten profiles as `(user_id, guild_id, score)`. A `scope` of `None` ranks everyone.
async fn top_ten(
    db: &SqlitePool,
    start: &str,
    end: &str,
    scope: Option<&str>,
) -> sqlx::Result<Vec<(String, String, i64)>> {
    sqlx::query_as(AssertSqlSafe(format!(
        "{STANDINGS} ORDER BY score DESC, user_id ASC LIMIT 10"
    )))
    .bind(start)
    .bind(end)
    .bind(scope)
    .fetch_all(db)
    .await
}

/// A profile's score and 1-based position, or `None` if it grew nothing that season.
async fn own_standing(
    db: &SqlitePool,
    start: &str,
    end: &str,
    scope: Option<&str>,
    user_id: &str,
    guild_id: &str,
) -> sqlx::Result<Option<(i64, usize)>> {
    let score: Option<i64> = sqlx::query_scalar(AssertSqlSafe(format!(
        "SELECT score FROM ({STANDINGS}) WHERE user_id = ?4 AND guild_id = ?5"
    )))
    .bind(start)
    .bind(end)
    .bind(scope)
    .bind(user_id)
    .bind(guild_id)
    .fetch_optional(db)
    .await?;
    let Some(score) = score else {
        return Ok(None);
    };
    let above: i64 = sqlx::query_scalar(AssertSqlSafe(format!(
        "SELECT COUNT(*) FROM ({STANDINGS}) WHERE score > ?4"
    )))
    .bind(start)
    .bind(end)
    .bind(scope)
    .bind(score)
    .fetch_one(db)
    .await?;
    Ok(Some((score, above as usize + 1)))
}

pub async fn run(cmd: &Cmd<'_>) -> CommandResult {
    let mut global = false;
    let mut previous = false;
    for option in cmd.interaction.data.options() {
        match (option.name, option.value) {
            ("scope", ResolvedValue::String(value)) => global = value == "global",
            ("period", ResolvedValue::String(value)) => previous = value == "previous",
            _ => {}
        }
    }

    // Resolving names can take longer than Discord's 3 second response window.
    cmd.defer().await?;

    let db = &cmd.bot.db;
    let (start, end) = bounds(time::now(), previous);
    let (start_at, end_at) = (time::format(start), time::format(end));
    let scope = (!global).then_some(cmd.guild.as_str());

    let top = top_ten(db, &start_at, &end_at, scope).await?;

    let season = start.format("%B %Y");
    let place = if global {
        "Global".to_string()
    } else {
        cmd.scope_name()
    };
    let title = format!("🗓️ {season} Season: {place} 🏆");

    if top.is_empty() {
        return cmd
            .edit(embed(
                title,
                "Nobody grew anything this season. The podium is wide open!",
                colors::NEUTRAL,
            ))
            .await;
    }

    let names = display_names(cmd.ctx, top.iter().map(|(user, _, _)| user.as_str())).await;
    let mut description = if previous {
        "Final standings:\n\n".to_string()
    } else {
        format!("Season ends {}.\n\n", time::relative(end))
    };
    for (i, ((user, guild, score), name)) in top.iter().zip(&names).enumerate() {
        let origin = if global {
            format!(" (from {})", guild_label(&cmd.ctx.cache, guild))
        } else {
            String::new()
        };
        let you = if *user == cmd.user && *guild == cmd.guild {
            " ← you"
        } else {
            ""
        };
        description.push_str(&format!(
            "{} **{}. {name}**: +{score} cm{origin}{you}\n",
            medal(i),
            i + 1
        ));
    }

    if !top
        .iter()
        .any(|(user, guild, _)| *user == cmd.user && *guild == cmd.guild)
        && let Some((score, rank)) =
            own_standing(db, &start_at, &end_at, scope, &cmd.user, &cmd.guild).await?
    {
        description.push_str(&format!(
            "⋯\n📍 **You: {}** with +{score} cm\n",
            ordinal(rank)
        ));
    }

    cmd.edit(
        embed(title, description, colors::GOLD).footer(CreateEmbedFooter::new(
            "Only cm you grew yourself count. Gifts and battles don't.",
        )),
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::{bounds, own_standing, top_ten};
    use crate::time;
    use sqlx::sqlite::SqlitePoolOptions;

    #[tokio::test]
    async fn standings_count_only_growth_earned_in_the_season() {
        // A single connection, since every in-memory connection is its own database.
        let db = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::raw_sql(include_str!(
            "../../migrations/20250309235354_initialize.sql"
        ))
        .execute(&db)
        .await
        .unwrap();
        sqlx::raw_sql(include_str!(
            "../../migrations/20250310000000_add_features.sql"
        ))
        .execute(&db)
        .await
        .unwrap();
        for (user, guild, change, kind, at) in [
            ("a", "g", 10, "grow", "2026-01-05 10:00:00"),
            ("a", "g", 3, "streak", "2026-01-05 10:00:00"),
            ("a", "g", 500, "gift_received", "2026-01-06 10:00:00"),
            ("a", "g", 900, "pvp_won", "2026-01-06 11:00:00"),
            ("b", "g", 20, "dotd", "2026-01-31 23:59:59"),
            ("b", "g", 99, "grow", "2026-02-01 00:00:00"),
            ("c", "g", 5, "grow", "2025-12-31 23:59:59"),
            ("c", "g", 1, "daily_bonus", "2026-01-01 00:00:00"),
            ("d", "other", 40, "community_pot", "2026-01-10 00:00:00"),
        ] {
            sqlx::query(
                "INSERT INTO length_history
                 (user_id, guild_id, length, growth_amount, growth_type, timestamp)
                 VALUES (?, ?, 0, ?, ?, ?)",
            )
            .bind(user)
            .bind(guild)
            .bind(change)
            .bind(kind)
            .bind(at)
            .execute(&db)
            .await
            .unwrap();
        }
        let (start, end) = ("2026-01-01 00:00:00", "2026-02-01 00:00:00");
        let row = |user: &str, guild: &str, score| (user.to_string(), guild.to_string(), score);

        let server = top_ten(&db, start, end, Some("g")).await.unwrap();
        assert_eq!(
            server,
            [row("b", "g", 20), row("a", "g", 13), row("c", "g", 1)]
        );

        let global = top_ten(&db, start, end, None).await.unwrap();
        assert_eq!(global[0], row("d", "other", 40));
        assert_eq!(global.len(), 4);

        let standing = |scope, user: &'static str| {
            let db = &db;
            async move {
                own_standing(db, start, end, scope, user, "g")
                    .await
                    .unwrap()
            }
        };
        assert_eq!(standing(Some("g"), "c").await, Some((1, 3)));
        assert_eq!(standing(None, "c").await, Some((1, 4)));
        assert_eq!(standing(None, "a").await, Some((13, 3)));
        assert_eq!(standing(Some("g"), "nobody").await, None);
    }

    #[test]
    fn seasons_are_utc_calendar_months() {
        let now = time::parse("2026-01-15 12:30:00").unwrap();
        let (start, end) = bounds(now, false);
        assert_eq!(time::format(start), "2026-01-01 00:00:00");
        assert_eq!(time::format(end), "2026-02-01 00:00:00");

        let (start, end) = bounds(now, true);
        assert_eq!(time::format(start), "2025-12-01 00:00:00");
        assert_eq!(time::format(end), "2026-01-01 00:00:00");
    }
}
