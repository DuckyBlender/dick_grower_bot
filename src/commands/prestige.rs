use crate::Bot;
use log::error;
use serenity::all::{
    ButtonStyle, CommandInteraction, ComponentInteraction, CreateActionRow, CreateButton,
    CreateEmbed, CreateEmbedFooter, CreateInteractionResponse, CreateInteractionResponseMessage,
};
use serenity::prelude::*;
use sqlx::{Pool, Row, Sqlite};

const BASE_REQUIREMENT_CM: i64 = 500;
const BASE_POINTS: i64 = 5;
const MAX_GROWTH_BONUS_PERCENT: i64 = 25;

pub fn required_length_for_level(prestige_level: i64) -> i64 {
    if prestige_level < 0 {
        return BASE_REQUIREMENT_CM;
    }
    u32::try_from(prestige_level)
        .ok()
        .and_then(|shift| BASE_REQUIREMENT_CM.checked_shl(shift))
        .unwrap_or(i64::MAX)
}

pub fn points_for_prestige(progress: i64, requirement: i64) -> i64 {
    if progress < requirement || requirement <= 0 {
        return 0;
    }
    let ratio = progress as f64 / requirement as f64;
    BASE_POINTS + (2.0 * ratio.log2()).floor().max(0.0) as i64
}

pub fn prestige_growth_bonus_percent(points: i64) -> i64 {
    points.clamp(0, MAX_GROWTH_BONUS_PERCENT)
}

pub fn add_prestige_bonus(multiplier: f64, bonus_percent: i64) -> f64 {
    multiplier + prestige_growth_bonus_percent(bonus_percent) as f64 / 100.0
}

#[derive(Debug, PartialEq, Eq)]
pub struct PrestigeOutcome {
    pub new_level: i64,
    pub points_earned: i64,
    pub total_points: i64,
}

#[derive(Debug)]
pub enum PrestigeError {
    StaleOrIneligible,
    Database(sqlx::Error),
}

impl std::fmt::Display for PrestigeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StaleOrIneligible => write!(formatter, "prestige confirmation is stale"),
            Self::Database(why) => write!(formatter, "prestige database error: {why}"),
        }
    }
}

impl std::error::Error for PrestigeError {}

impl From<sqlx::Error> for PrestigeError {
    fn from(value: sqlx::Error) -> Self {
        Self::Database(value)
    }
}

pub async fn perform_prestige(
    database: &Pool<Sqlite>,
    user_id: &str,
    guild_id: &str,
    expected_level: i64,
) -> Result<PrestigeOutcome, PrestigeError> {
    let row = sqlx::query(
        "SELECT length, prestige_level, prestige_points, prestige_progress
         FROM dicks WHERE user_id = ? AND guild_id = ?",
    )
    .bind(user_id)
    .bind(guild_id)
    .fetch_optional(database)
    .await?
    .ok_or(PrestigeError::StaleOrIneligible)?;
    let length: i64 = row.try_get("length")?;
    let level: i64 = row.try_get("prestige_level")?;
    let old_points: i64 = row.try_get("prestige_points")?;
    let progress: i64 = row.try_get("prestige_progress")?;
    let requirement = required_length_for_level(level);
    if level != expected_level || length < requirement || progress < requirement {
        return Err(PrestigeError::StaleOrIneligible);
    }

    let points_earned = points_for_prestige(progress, requirement);
    let total_points = old_points.saturating_add(points_earned);
    let new_level = level.saturating_add(1);
    let mut tx = database.begin().await?;
    let update = sqlx::query(
        "UPDATE dicks
         SET length = 0, prestige_progress = 0,
             prestige_level = ?, prestige_points = ?
         WHERE user_id = ? AND guild_id = ?
           AND prestige_level = ? AND length >= ? AND prestige_progress >= ?",
    )
    .bind(new_level)
    .bind(total_points)
    .bind(user_id)
    .bind(guild_id)
    .bind(expected_level)
    .bind(requirement)
    .bind(requirement)
    .execute(&mut *tx)
    .await?;
    if update.rows_affected() != 1 {
        tx.rollback().await?;
        return Err(PrestigeError::StaleOrIneligible);
    }

    sqlx::query(
        "INSERT INTO prestige_history
            (user_id, guild_id, prestige_level, points_earned,
             length_before_reset, progress_before_reset)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(user_id)
    .bind(guild_id)
    .bind(new_level)
    .bind(points_earned)
    .bind(length)
    .bind(progress)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO length_history
            (user_id, guild_id, length, growth_amount, growth_type)
         VALUES (?, ?, 0, ?, 'prestige')",
    )
    .bind(user_id)
    .bind(guild_id)
    .bind(-length)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    Ok(PrestigeOutcome {
        new_level,
        points_earned,
        total_points,
    })
}

pub async fn growth_bonus_for_profile(bot: &Bot, user_id: &str, guild_id: &str) -> i64 {
    sqlx::query("SELECT prestige_points FROM dicks WHERE user_id = ? AND guild_id = ?")
        .bind(user_id)
        .bind(guild_id)
        .fetch_optional(&bot.database)
        .await
        .ok()
        .flatten()
        .and_then(|row| row.try_get::<i64, _>("prestige_points").ok())
        .map(prestige_growth_bonus_percent)
        .unwrap_or(0)
}

pub async fn handle_prestige_command(
    ctx: &Context,
    command: &CommandInteraction,
) -> Result<(), serenity::Error> {
    let data = ctx.data.read().await;
    let bot = data.get::<Bot>().unwrap();
    let user_id = command.user.id.to_string();
    let guild_id = command.guild_id.expect("guild command").to_string();

    let row = match sqlx::query(
        "SELECT length, prestige_level, prestige_points, prestige_progress
         FROM dicks WHERE user_id = ? AND guild_id = ?",
    )
    .bind(&user_id)
    .bind(&guild_id)
    .fetch_optional(&bot.database)
    .await
    {
        Ok(Some(row)) => row,
        Ok(None) => {
            return command
                .create_response(
                    &ctx.http,
                    CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new()
                            .content("Use `/grow` before trying to prestige.")
                            .ephemeral(true),
                    ),
                )
                .await;
        }
        Err(why) => {
            error!("Failed to read prestige profile: {:?}", why);
            return command
                .create_response(
                    &ctx.http,
                    CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new()
                            .content("Prestige is unavailable right now.")
                            .ephemeral(true),
                    ),
                )
                .await;
        }
    };

    let length: i64 = row.try_get("length").unwrap_or_default();
    let level: i64 = row.try_get("prestige_level").unwrap_or_default();
    let points: i64 = row.try_get("prestige_points").unwrap_or_default();
    let progress: i64 = row.try_get("prestige_progress").unwrap_or_default();
    let requirement = required_length_for_level(level);

    if length < requirement || progress < requirement {
        let description = format!(
            "Prestige **{} → {}** requires both:\n\n• Current length: **{} / {} cm**\n• Earned progress: **{} / {} cm**\n\nGifts and PvP transfers can change length, but they do not count as earned progress.",
            level,
            level + 1,
            length,
            requirement,
            progress,
            requirement
        );
        return command
            .create_response(
                &ctx.http,
                CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .add_embed(
                            CreateEmbed::new()
                                .title("🍆 Not Ready to Prestige")
                                .description(description)
                                .color(0xFF5733),
                        )
                        .ephemeral(true),
                ),
            )
            .await;
    }

    let earned = points_for_prestige(progress, requirement);
    let new_points = points.saturating_add(earned);
    let new_bonus = prestige_growth_bonus_percent(new_points);
    let confirm = CreateButton::new(format!("prestige_confirm:{user_id}:{guild_id}:{level}"))
        .label("Confirm Prestige")
        .style(ButtonStyle::Danger)
        .emoji('🔥');
    let cancel = CreateButton::new(format!("prestige_cancel:{user_id}"))
        .label("Cancel")
        .style(ButtonStyle::Secondary);

    command
        .create_response(
            &ctx.http,
            CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .add_embed(
                        CreateEmbed::new()
                            .title("⚠️ Confirm Prestige")
                            .description(format!(
                                "This permanently resets your **{} cm** current length and **{} cm** earned progress to zero.\n\nYou gain **{} PP**, reach prestige **{}**, and your `/grow` bonus becomes **+{}%**. Your season score and all other stats stay intact.",
                                length,
                                progress,
                                earned,
                                level + 1,
                                new_bonus
                            ))
                            .color(0xE67E22)
                            .footer(CreateEmbedFooter::new(
                                "This cannot be undone after confirmation.",
                            )),
                    )
                    .components(vec![CreateActionRow::Buttons(vec![confirm, cancel])])
                    .ephemeral(true),
            ),
        )
        .await
}

pub async fn handle_prestige_component(
    ctx: &Context,
    component: &ComponentInteraction,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let parts: Vec<&str> = component.data.custom_id.split(':').collect();
    let owner_id = parts.get(1).copied().unwrap_or_default();
    if owner_id != component.user.id.to_string() {
        component
            .create_response(
                &ctx.http,
                CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .content("Only the player who opened this confirmation can use it.")
                        .ephemeral(true),
                ),
            )
            .await?;
        return Ok(());
    }

    if component.data.custom_id.starts_with("prestige_cancel:") {
        component
            .create_response(
                &ctx.http,
                CreateInteractionResponse::UpdateMessage(
                    CreateInteractionResponseMessage::new()
                        .content("Prestige cancelled. Nothing was reset.")
                        .components(vec![]),
                ),
            )
            .await?;
        return Ok(());
    }

    let guild_id = parts.get(2).copied().unwrap_or_default();
    let expected_level = parts
        .get(3)
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(-1);
    if component.guild_id.map(|id| id.to_string()).as_deref() != Some(guild_id) {
        component
            .create_response(
                &ctx.http,
                CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .content("This prestige confirmation belongs to another server.")
                        .ephemeral(true),
                ),
            )
            .await?;
        return Ok(());
    }

    let data = ctx.data.read().await;
    let bot = data.get::<Bot>().unwrap();
    let outcome = match perform_prestige(&bot.database, owner_id, guild_id, expected_level).await {
        Ok(outcome) => outcome,
        Err(PrestigeError::StaleOrIneligible) => {
            component
                .create_response(
                    &ctx.http,
                    CreateInteractionResponse::UpdateMessage(
                        CreateInteractionResponseMessage::new()
                            .content("This prestige confirmation was already used or expired. Run `/prestige` again.")
                            .components(vec![]),
                    ),
                )
                .await?;
            return Ok(());
        }
        Err(why) => return Err(Box::new(why)),
    };

    component
        .create_response(
            &ctx.http,
            CreateInteractionResponse::UpdateMessage(
                CreateInteractionResponseMessage::new()
                    .add_embed(
                        CreateEmbed::new()
                            .title("🏆 Prestige Complete!")
                            .description(format!(
                                "You reached prestige **{}** and earned **{} PP**.\n\nTotal PP: **{}**\nPermanent `/grow` bonus: **+{}%**\nNext requirement: **{} cm**\n\nYour current length is now **0 cm**, but your season score was preserved.",
                                outcome.new_level,
                                outcome.points_earned,
                                outcome.total_points,
                                prestige_growth_bonus_percent(outcome.total_points),
                                required_length_for_level(outcome.new_level)
                            ))
                            .color(0x2ECC71),
                    )
                    .components(vec![]),
            ),
        )
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    #[test]
    fn threshold_doubles_and_saturates() {
        assert_eq!(required_length_for_level(0), 500);
        assert_eq!(required_length_for_level(1), 1_000);
        assert_eq!(required_length_for_level(5), 16_000);
        assert_eq!(required_length_for_level(100), i64::MAX);
    }

    #[test]
    fn points_reward_full_and_partial_doublings() {
        assert_eq!(points_for_prestige(499, 500), 0);
        assert_eq!(points_for_prestige(500, 500), 5);
        assert_eq!(points_for_prestige(750, 500), 6);
        assert_eq!(points_for_prestige(1_000, 500), 7);
        assert_eq!(points_for_prestige(19_174, 500), 15);
    }

    #[test]
    fn growth_bonus_is_capped() {
        assert_eq!(prestige_growth_bonus_percent(-5), 0);
        assert_eq!(prestige_growth_bonus_percent(15), 15);
        assert_eq!(prestige_growth_bonus_percent(100), 25);
        assert!((add_prestige_bonus(1.75, 15) - 1.90).abs() < f64::EPSILON);
        assert!((add_prestige_bonus(1.75, 100) - 2.00).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn prestige_is_atomic_preserves_season_score_and_rejects_reuse() {
        let database = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        for statement in [
            "CREATE TABLE dicks (
                user_id TEXT NOT NULL, guild_id TEXT NOT NULL, length INTEGER NOT NULL,
                prestige_level INTEGER NOT NULL DEFAULT 0,
                prestige_points INTEGER NOT NULL DEFAULT 0,
                prestige_progress INTEGER NOT NULL DEFAULT 0,
                UNIQUE(user_id, guild_id)
            )",
            "CREATE TABLE prestige_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT, user_id TEXT NOT NULL, guild_id TEXT NOT NULL,
                prestige_level INTEGER NOT NULL, points_earned INTEGER NOT NULL,
                length_before_reset INTEGER NOT NULL, progress_before_reset INTEGER NOT NULL,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            )",
            "CREATE TABLE length_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT, user_id TEXT NOT NULL, guild_id TEXT NOT NULL,
                length INTEGER NOT NULL, growth_amount INTEGER NOT NULL,
                timestamp TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, growth_type TEXT NOT NULL
            )",
            "CREATE TABLE season_scores (
                season_id INTEGER NOT NULL, user_id TEXT NOT NULL, guild_id TEXT NOT NULL,
                score INTEGER NOT NULL, PRIMARY KEY(season_id, user_id, guild_id)
            )",
            "INSERT INTO dicks
                (user_id, guild_id, length, prestige_progress)
             VALUES ('u1', 'g1', 1000, 1000)",
            "INSERT INTO season_scores VALUES (1, 'u1', 'g1', 123)",
        ] {
            sqlx::query(statement).execute(&database).await.unwrap();
        }

        let outcome = perform_prestige(&database, "u1", "g1", 0).await.unwrap();
        assert_eq!(
            outcome,
            PrestigeOutcome {
                new_level: 1,
                points_earned: 7,
                total_points: 7
            }
        );
        let row = sqlx::query(
            "SELECT length, prestige_progress, prestige_level FROM dicks WHERE user_id = 'u1'",
        )
        .fetch_one(&database)
        .await
        .unwrap();
        assert_eq!(row.try_get::<i64, _>("length").unwrap(), 0);
        assert_eq!(row.try_get::<i64, _>("prestige_progress").unwrap(), 0);
        assert_eq!(row.try_get::<i64, _>("prestige_level").unwrap(), 1);
        let season_score =
            sqlx::query_scalar::<_, i64>("SELECT score FROM season_scores WHERE user_id = 'u1'")
                .fetch_one(&database)
                .await
                .unwrap();
        assert_eq!(season_score, 123);
        assert!(matches!(
            perform_prestige(&database, "u1", "g1", 0).await,
            Err(PrestigeError::StaleOrIneligible)
        ));
    }
}
