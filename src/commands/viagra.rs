use crate::commands::events::{GlobalEvent, active_event};
use crate::commands::{Cmd, CommandResult};
use crate::db;
use crate::time;
use crate::utils::{colors, embed};
use chrono::{Duration, NaiveDateTime};
use serenity::all::{CreateCommand, CreateEmbedFooter};

pub const BOOST_PERCENT: i64 = 20;
pub const COOLDOWN_HOURS: i64 = 20;
pub const DURATION_HOURS: i64 = 6;

/// When the viagra effect ends, if it's still active.
pub fn active_until(active_until: Option<&str>) -> Option<NaiveDateTime> {
    active_until
        .and_then(time::parse)
        .filter(|&until| until > time::now())
}

/// When viagra can be taken again, if it's still on cooldown.
pub fn cooldown_ends(last_used: Option<&str>) -> Option<NaiveDateTime> {
    last_used
        .and_then(time::parse)
        .map(|used| used + Duration::hours(COOLDOWN_HOURS))
        .filter(|&ready| ready > time::now())
}

pub fn register() -> CreateCommand {
    CreateCommand::new("viagra").description(format!(
        "Boost your growth by {BOOST_PERCENT}% for {DURATION_HOURS} hours ({COOLDOWN_HOURS} hour cooldown)"
    ))
}

pub async fn run(cmd: &Cmd<'_>) -> CommandResult {
    db::ensure_user(&cmd.bot.db, &cmd.user, &cmd.guild).await?;

    let status = sqlx::query!(
        "SELECT viagra_last_used, viagra_active_until FROM dicks WHERE user_id = ? AND guild_id = ?",
        cmd.user,
        cmd.guild
    )
    .fetch_one(&cmd.bot.db)
    .await?;

    if let Some(until) = active_until(status.viagra_active_until.as_deref()) {
        return cmd
            .reply_ephemeral(
                embed(
                    "💊 Viagra Already Active!",
                    format!(
                        "Your viagra is still working its magic! 🔥\n\nYou'll get **+{BOOST_PERCENT}% growth** until it wears off {}. No need to double dose!",
                        time::relative(until)
                    ),
                    colors::INFO,
                )
                .footer(CreateEmbedFooter::new(
                    "Patience, young grasshopper. Good things come to those who wait.",
                )),
            )
            .await;
    }

    if let Some(ready) = cooldown_ends(status.viagra_last_used.as_deref()) {
        return reply_cooldown(cmd, ready).await;
    }

    let event = active_event(cmd.bot).await?;
    let event_hours = event.as_ref().and_then(GlobalEvent::viagra_duration_hours);
    let duration_hours = event_hours.unwrap_or(DURATION_HOURS);

    let now = time::now();
    let effect_ends = now + Duration::hours(duration_hours);
    let next_available = now + Duration::hours(COOLDOWN_HOURS);
    let (now_str, effect_ends_str) = (time::format(now), time::format(effect_ends));
    let cooldown_cutoff = time::format(now - Duration::hours(COOLDOWN_HOURS));

    // The cooldown is re-checked in the UPDATE so concurrent invocations can't both succeed.
    let claimed = sqlx::query!(
        "UPDATE dicks SET viagra_last_used = ?, viagra_active_until = ?
         WHERE user_id = ? AND guild_id = ?
           AND (viagra_last_used IS NULL OR viagra_last_used <= ?)",
        now_str,
        effect_ends_str,
        cmd.user,
        cmd.guild,
        cooldown_cutoff
    )
    .execute(&cmd.bot.db)
    .await?
    .rows_affected()
        > 0;
    if !claimed {
        return reply_cooldown(cmd, next_available).await;
    }

    let event_note = match (event_hours, &event) {
        (Some(_), Some(event)) => format!(" (🌍 {})", event.name),
        _ => String::new(),
    };

    cmd.reply(
        embed(
            "💊 VIAGRA ACTIVATED! 🔥",
            format!(
                "You've taken the magical blue pill! 💎\n\n\
                 • **+{BOOST_PERCENT}%** growth on every /grow\n\
                 • Lasts **{duration_hours} hours**{event_note}, wears off {}\n\
                 • Next dose available {}\n\n\
                 Your dick is now supercharged! Get growing! 🚀",
                time::relative(effect_ends),
                time::relative(next_available)
            ),
            colors::INFO,
        )
        .footer(CreateEmbedFooter::new(
            "Warning: Side effects may include uncontrollable confidence and swagger.",
        )),
    )
    .await
}

async fn reply_cooldown(cmd: &Cmd<'_>, ready: NaiveDateTime) -> CommandResult {
    cmd.reply_ephemeral(
        embed(
            "🚫 Viagra Cooldown Active",
            format!(
                "Whoa there, speedster! Your body needs time to recover from the last enhancement session.\n\nNext dose available {}.",
                time::relative(ready)
            ),
            colors::WARNING,
        )
        .footer(CreateEmbedFooter::new(
            "Remember: Too much enhancement can lead to... complications.",
        )),
    )
    .await
}
