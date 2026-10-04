use crate::commands::events::{GlobalEvent, active_event, add_to_community_pot};
use crate::commands::viagra;
use crate::commands::{Cmd, CommandResult};
use crate::db::{self, History};
use crate::time;
use crate::utils::{colors, embed, ordinal, pluralize};
use chrono::{Days, Duration, NaiveDate, Utc};
use rand::RngExt;
use rand::seq::IndexedRandom;
use serenity::all::{CreateCommand, CreateEmbedFooter};
use sqlx::SqlitePool;

const BASE_GROWTH_RANGE: (i64, i64) = (1, 10);
pub const DEFAULT_COOLDOWN_MINUTES: i64 = 60;
const MAX_STREAK_REWARD_CM: i64 = 5;

const FOOTERS: &[&str] = &[
    "Remember: it's not about the size, it's about... actually, it is about the size.",
    "Your ruler is judging you.",
    "Even tiny steps are still forward progress.",
    "The pen is mighty, but the dick is mightier.",
    "Grow slow, go low, stay low.",
    "Nature hates a vacuum, but loves a full one.",
    "A journey of a thousand miles begins with a single /grow.",
    "With great length comes great responsibility.",
    "You're doing great!",
    "If you can measure it, you can improve it.",
    "The early bird gets the worm. The big dick gets respect.",
    "Rome wasn't built in a day, and neither was your dick.",
    "Stay hungry, stay humble, stay growing.",
    "What goes up must come down... eventually.",
    "In a time of uncertainty, /grow.",
    "Trust the process. Trust the gains.",
    "Big things come to those who wait... and /grow daily.",
    "The only bad measurement is no measurement.",
    "Keep it between the sheets and in the database.",
    "Your dick is a garden. Water it daily.",
];

pub fn cooldown_minutes(event: Option<&GlobalEvent>) -> i64 {
    event
        .and_then(GlobalEvent::grow_cooldown_minutes)
        .unwrap_or(DEFAULT_COOLDOWN_MINUTES)
}

/// Logarithmic streak reward: 1 cm on day one, slowly rising to a cap.
fn streak_reward(streak: i64) -> i64 {
    ((1.0 + (streak as f64).ln() * 0.8).round() as i64).clamp(1, MAX_STREAK_REWARD_CM)
}

pub fn register() -> CreateCommand {
    CreateCommand::new("grow").description("Grow your cucumber")
}

pub async fn run(cmd: &Cmd<'_>) -> CommandResult {
    let db = &cmd.bot.db;
    db::ensure_user(db, &cmd.user, &cmd.guild).await?;

    let event = active_event(cmd.bot).await?;
    let cooldown = cooldown_minutes(event.as_ref());
    let perks = sqlx::query!(
        "SELECT daily_growth_boost_percent, daily_lucky_rolls, viagra_active_until
         FROM dicks WHERE user_id = ? AND guild_id = ?",
        cmd.user,
        cmd.guild
    )
    .fetch_one(db)
    .await?;

    let lucky_roll = perks.daily_lucky_rolls > 0;
    let event_double_roll = event.as_ref().is_some_and(GlobalEvent::rolls_growth_twice);
    let (min, max) = event
        .as_ref()
        .and_then(GlobalEvent::growth_range)
        .unwrap_or(BASE_GROWTH_RANGE);
    let roll = {
        let mut rng = rand::rng();
        let first = rng.random_range(min..=max);
        if lucky_roll || event_double_roll {
            first.max(rng.random_range(min..=max))
        } else {
            first
        }
    };

    let mut boosts = Vec::new();
    let mut bonus_percent = 0;
    if lucky_roll {
        boosts.push("🍀 Lucky roll".to_string());
    }
    if viagra::active_until(perks.viagra_active_until.as_deref()).is_some() {
        bonus_percent += viagra::BOOST_PERCENT;
        boosts.push(format!("💊 Viagra +{}%", viagra::BOOST_PERCENT));
    }
    if perks.daily_growth_boost_percent > 0 {
        bonus_percent += perks.daily_growth_boost_percent;
        boosts.push(format!("⚡ Daily +{}%", perks.daily_growth_boost_percent));
    }
    if let Some(event) = &event {
        if event_double_roll {
            boosts.push(format!("🌍 {} (double roll)", event.name));
        }
        if let Some(percent) = event.growth_bonus_percent() {
            bonus_percent += percent;
            boosts.push(format!("🌍 {} +{percent}%", event.name));
        }
    }

    let mut growth = (roll as f64 * (100 + bonus_percent) as f64 / 100.0).round() as i64;
    if let Some(jackpot) = event.as_ref().and_then(GlobalEvent::roll_jackpot) {
        growth += jackpot;
        boosts.push(format!("🎰 Jackpot +{jackpot} cm"));
    }

    // The cooldown check and the growth are a single statement, so spamming /grow can't
    // sneak in extra growths. One-shot perks are consumed in the same statement.
    let lucky_used = i64::from(lucky_roll);
    let cooldown_modifier = format!("-{cooldown} minutes");
    let mut new_length = sqlx::query_scalar!(
        "UPDATE dicks
         SET length = length + ?, last_grow = datetime('now'), growth_count = growth_count + 1,
             daily_growth_boost_percent = 0, daily_lucky_rolls = MAX(daily_lucky_rolls - ?, 0)
         WHERE user_id = ? AND guild_id = ? AND last_grow <= datetime('now', ?)
         RETURNING length",
        growth,
        lucky_used,
        cmd.user,
        cmd.guild,
        cooldown_modifier
    )
    .fetch_optional(db)
    .await?;

    if new_length.is_none() {
        new_length = sqlx::query_scalar!(
            "UPDATE dicks
             SET length = length + ?, last_grow = datetime('now'), growth_count = growth_count + 1,
                 daily_growth_boost_percent = 0, daily_lucky_rolls = MAX(daily_lucky_rolls - ?, 0),
                 daily_cooldown_skips = daily_cooldown_skips - 1
             WHERE user_id = ? AND guild_id = ? AND daily_cooldown_skips > 0
             RETURNING length",
            growth,
            lucky_used,
            cmd.user,
            cmd.guild
        )
        .fetch_optional(db)
        .await?;
        if new_length.is_some() {
            boosts.insert(0, "⏩ Cooldown skip".to_string());
        }
    }

    let Some(mut new_length) = new_length else {
        return reply_on_cooldown(cmd, cooldown).await;
    };
    db::log_history(db, &cmd.user, &cmd.guild, new_length, growth, History::Grow).await?;

    if let Some(event) = &event
        && let Some(amount) = event.community_pot_cm_per_grow()
    {
        add_to_community_pot(cmd.bot, event.id, amount).await?;
        boosts.push(format!("🏺 {} pot +{amount} cm", event.name));
    }

    if let Some(streak) = advance_streak(db, &cmd.user, &cmd.guild).await? {
        if streak.used_saver {
            boosts.push("🛟 Streak saver".to_string());
        }
        boosts.push(format!(
            "🔥 {} streak +{} cm",
            pluralize(streak.days, "day", "days"),
            streak.reward
        ));
        new_length = streak.new_length;
    }

    let rank = db::guild_rank(db, &cmd.guild, new_length).await?;
    let next_grow = time::now() + Duration::minutes(cooldown);

    let (title, flavor, color) = match growth {
        11.. => (
            "🚀 INCREDIBLE GROWTH!",
            "Careful, you might trip over it soon!",
            0x00FF00,
        ),
        8..=10 => (
            "🔥 Impressive Growth!",
            "Keep up the good work, size king!",
            0x33FF33,
        ),
        4..=7 => ("🌱 Solid Growth", "Every centimeter counts!", 0x66FF66),
        _ => (
            "📏 Modest Growth",
            "Small steps lead to big achievements!",
            0x99FF99,
        ),
    };
    let boosts = if boosts.is_empty() {
        String::new()
    } else {
        format!("\n**Boosts:** {}", boosts.join(" · "))
    };
    let footer = *FOOTERS
        .choose(&mut rand::rng())
        .expect("footers are not empty");

    cmd.reply(
        embed(
            title,
            format!(
                "Your dick grew by **+{growth} cm** and is now **{new_length} cm** long!{boosts}\n\n\
                 🏅 Server rank: **{}**\n\
                 ⏰ Next grow: {}\n\n\
                 *{flavor}*",
                ordinal(rank),
                time::relative(next_grow)
            ),
            color,
        )
        .footer(CreateEmbedFooter::new(footer)),
    )
    .await
}

async fn reply_on_cooldown(cmd: &Cmd<'_>, cooldown: i64) -> CommandResult {
    let last_grow = sqlx::query_scalar!(
        "SELECT last_grow FROM dicks WHERE user_id = ? AND guild_id = ?",
        cmd.user,
        cmd.guild
    )
    .fetch_one(&cmd.bot.db)
    .await?;
    let ready = time::parse(&last_grow).unwrap_or_else(time::now) + Duration::minutes(cooldown);

    cmd.reply_ephemeral(
        embed(
            "🕒 Hold up, speedy!",
            format!(
                "You've already played with your dick recently! Try again {}.\n\nExcessive stimulation might cause injuries, you know?",
                time::relative(ready)
            ),
            colors::WARNING,
        )
        .footer(CreateEmbedFooter::new(
            "Tip: /daily can give you a cooldown skip.",
        )),
    )
    .await
}

struct StreakUpdate {
    days: i64,
    reward: i64,
    used_saver: bool,
    new_length: i64,
}

/// Advances the daily growth streak on the first grow of each UTC day and pays its reward.
async fn advance_streak(
    db: &SqlitePool,
    user_id: &str,
    guild_id: &str,
) -> sqlx::Result<Option<StreakUpdate>> {
    let row = sqlx::query!(
        "SELECT daily_streak, last_streak_date, daily_streak_savers
         FROM dicks WHERE user_id = ? AND guild_id = ?",
        user_id,
        guild_id
    )
    .fetch_one(db)
    .await?;

    let today = Utc::now().date_naive();
    let last_date = row
        .last_streak_date
        .as_deref()
        .and_then(|date| NaiveDate::parse_from_str(date, "%Y-%m-%d").ok());
    if last_date == Some(today) {
        return Ok(None);
    }

    let continues = last_date == today.checked_sub_days(Days::new(1));
    let used_saver = !continues
        && last_date == today.checked_sub_days(Days::new(2))
        && row.daily_streak > 0
        && row.daily_streak_savers > 0;
    let days = if continues || used_saver {
        row.daily_streak + 1
    } else {
        1
    };
    let reward = streak_reward(days);
    let today_str = today.format("%Y-%m-%d").to_string();
    let saver_used = i64::from(used_saver);

    let new_length = sqlx::query_scalar!(
        "UPDATE dicks
         SET daily_streak = ?, best_daily_streak = MAX(best_daily_streak, ?),
             last_streak_date = ?, streak_last_claimed = datetime('now'),
             daily_streak_savers = daily_streak_savers - ?, length = length + ?
         WHERE user_id = ? AND guild_id = ? AND last_streak_date IS NOT ?
         RETURNING length",
        days,
        days,
        today_str,
        saver_used,
        reward,
        user_id,
        guild_id,
        today_str
    )
    .fetch_optional(db)
    .await?;
    let Some(new_length) = new_length else {
        return Ok(None);
    };

    db::log_history(db, user_id, guild_id, new_length, reward, History::Streak).await?;
    Ok(Some(StreakUpdate {
        days,
        reward,
        used_saver,
        new_length,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn streak_reward_curve() {
        let cases = [
            (1, 1),
            (3, 2),
            (7, 3),
            (14, 3),
            (30, 4),
            (60, 4),
            (100, 5),
            (1000, 5),
        ];
        for (streak, expected) in cases {
            assert_eq!(streak_reward(streak), expected, "streak {streak}");
        }
    }
}
