use crate::commands::{Cmd, CommandResult};
use crate::db::{self, History};
use crate::time;
use crate::utils::{colors, embed};
use rand::RngExt;
use rand::seq::IndexedRandom;
use serenity::all::{CreateCommand, CreateEmbedFooter};

pub const NEXT_GROWTH_BOOST_PERCENT: i64 = 50;
const BONUS_CM_MIN: i64 = 5;
const BONUS_CM_MAX: i64 = 15;

#[derive(Clone, Copy)]
enum Reward {
    BonusCm,
    GrowthBoost,
    CooldownSkip,
    StreakSaver,
    LuckyRoll,
}

/// Relative odds of each daily reward.
const REWARD_WEIGHTS: [(Reward, u32); 5] = [
    (Reward::BonusCm, 1),
    (Reward::GrowthBoost, 1),
    (Reward::CooldownSkip, 1),
    (Reward::StreakSaver, 1),
    (Reward::LuckyRoll, 1),
];

pub fn register() -> CreateCommand {
    CreateCommand::new("daily").description("Claim a once-a-day random perk")
}

pub async fn run(cmd: &Cmd<'_>) -> CommandResult {
    let db = &cmd.bot.db;
    db::ensure_user(db, &cmd.user, &cmd.guild).await?;

    // Claiming and checking happen in one statement so the reward can't be claimed twice.
    let claimed = sqlx::query!(
        "UPDATE dicks SET daily_last_claimed = datetime('now')
         WHERE user_id = ? AND guild_id = ?
           AND (daily_last_claimed IS NULL OR date(daily_last_claimed) < date('now'))",
        cmd.user,
        cmd.guild
    )
    .execute(db)
    .await?
    .rows_affected()
        > 0;

    if !claimed {
        return cmd
            .reply_ephemeral(embed(
                "🕒 Daily Already Claimed",
                format!(
                    "You've already grabbed today's daily reward.\n\nCome back {} for another suspicious package.",
                    time::relative(time::next_utc_midnight())
                ),
                colors::WARNING,
            ))
            .await;
    }

    let (reward, _) = *REWARD_WEIGHTS
        .choose_weighted(&mut rand::rng(), |&(_, weight)| weight)
        .expect("reward weights are valid");

    let (title, description, color) = match reward {
        Reward::BonusCm => {
            let bonus = rand::rng().random_range(BONUS_CM_MIN..=BONUS_CM_MAX);
            let new_length = sqlx::query_scalar!(
                "UPDATE dicks SET length = length + ? WHERE user_id = ? AND guild_id = ?
                 RETURNING length",
                bonus,
                cmd.user,
                cmd.guild
            )
            .fetch_one(db)
            .await?;
            db::log_history(
                db,
                &cmd.user,
                &cmd.guild,
                new_length,
                bonus,
                History::DailyBonus,
            )
            .await?;
            (
                "🎁 Daily Bonus Claimed!",
                format!(
                    "You found **+{bonus} cm** in today's package!\n\nYour new length is **{new_length} cm**."
                ),
                colors::SUCCESS,
            )
        }
        Reward::GrowthBoost => {
            sqlx::query!(
                "UPDATE dicks SET daily_growth_boost_percent = ? WHERE user_id = ? AND guild_id = ?",
                NEXT_GROWTH_BOOST_PERCENT,
                cmd.user,
                cmd.guild
            )
            .execute(db)
            .await?;
            (
                "⚡ Daily Boost Claimed!",
                format!("Your next /grow gets **+{NEXT_GROWTH_BOOST_PERCENT}% growth**."),
                colors::INFO,
            )
        }
        Reward::CooldownSkip => {
            sqlx::query!(
                "UPDATE dicks SET daily_cooldown_skips = daily_cooldown_skips + 1
                 WHERE user_id = ? AND guild_id = ?",
                cmd.user,
                cmd.guild
            )
            .execute(db)
            .await?;
            (
                "⏩ Cooldown Skip Claimed!",
                "Your next /grow while on cooldown will ignore the cooldown.".to_string(),
                0x1ABC9C,
            )
        }
        Reward::StreakSaver => {
            sqlx::query!(
                "UPDATE dicks SET daily_streak_savers = daily_streak_savers + 1
                 WHERE user_id = ? AND guild_id = ?",
                cmd.user,
                cmd.guild
            )
            .execute(db)
            .await?;
            (
                "🛟 Streak Saver Claimed!",
                "Your daily growth streak can survive one missed UTC day.".to_string(),
                0xE67E22,
            )
        }
        Reward::LuckyRoll => {
            sqlx::query!(
                "UPDATE dicks SET daily_lucky_rolls = daily_lucky_rolls + 1
                 WHERE user_id = ? AND guild_id = ?",
                cmd.user,
                cmd.guild
            )
            .execute(db)
            .await?;
            (
                "🍀 Lucky Roll Claimed!",
                "Your next /grow rolls twice and keeps the better result.".to_string(),
                colors::SUCCESS,
            )
        }
    };

    cmd.reply(
        embed(title, description, color).footer(CreateEmbedFooter::new(
            "Daily rewards reset at midnight UTC. Check your perks with /stats.",
        )),
    )
    .await
}
