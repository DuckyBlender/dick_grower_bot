use crate::Bot;
use crate::commands::{Cmd, CommandResult};
use crate::db::{self, History};
use crate::time;
use crate::utils::{colors, embed};
use chrono::{DateTime, Duration, NaiveDateTime, Utc};
use log::{error, info, warn};
use rand::RngExt;
use rand::seq::IndexedRandom;
use serenity::all::{ActivityData, Context, CreateCommand, CreateEmbedFooter};
use std::sync::Arc;

/// Events are rolled at every UTC boundary of this many hours and last until the next one.
const EVENT_PERIOD_HOURS: i64 = 4;
/// Ticking slightly after the boundary guarantees the previous event has ended.
const TICK_DELAY: std::time::Duration = std::time::Duration::from_secs(5);
const ACTIVATION_CHANCE: (u32, u32) = (1, 2);

const GROWTH_BONUS_PERCENT: i64 = 25;
const LOWER_COOLDOWN_MINUTES: i64 = 30;
const LONGER_VIAGRA_HOURS: i64 = 12;
const COMPACT_GROWTH_RANGE: (i64, i64) = (1, 5);
const COMPACT_GROWTH_COOLDOWN_MINUTES: i64 = 15;
const JACKPOT_EXTRA_CM: i64 = 25;
const JACKPOT_CHANCE: (u32, u32) = (1, 10);
const COMMUNITY_POT_CM_PER_GROW: i64 = 1;

/// Relative odds of each event being picked once one starts.
const EVENT_WEIGHTS: [(EventKind, u32); 7] = [
    (EventKind::GrowthBonus, 1),
    (EventKind::LowerCooldown, 1),
    (EventKind::LongerViagra, 1),
    (EventKind::DoubleGrowthRoll, 1),
    (EventKind::CompactGrowth, 1),
    (EventKind::JackpotGrowth, 1),
    (EventKind::CommunityPot, 1),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EventKind {
    GrowthBonus,
    LowerCooldown,
    LongerViagra,
    DoubleGrowthRoll,
    CompactGrowth,
    JackpotGrowth,
    CommunityPot,
}

impl EventKind {
    fn as_str(self) -> &'static str {
        match self {
            EventKind::GrowthBonus => "growth_bonus",
            EventKind::LowerCooldown => "lower_cooldown",
            EventKind::LongerViagra => "longer_viagra",
            EventKind::DoubleGrowthRoll => "double_growth_roll",
            EventKind::CompactGrowth => "compact_growth",
            EventKind::JackpotGrowth => "jackpot_growth",
            EventKind::CommunityPot => "community_pot",
        }
    }

    fn from_str(value: &str) -> Option<Self> {
        EVENT_WEIGHTS
            .iter()
            .map(|&(kind, _)| kind)
            .find(|kind| kind.as_str() == value)
    }

    fn name(self) -> &'static str {
        match self {
            EventKind::GrowthBonus => "Growth Surge",
            EventKind::LowerCooldown => "Fast Hands",
            EventKind::LongerViagra => "Extended Pharmacy Hours",
            EventKind::DoubleGrowthRoll => "Double Trouble",
            EventKind::CompactGrowth => "Quick Sprouts",
            EventKind::JackpotGrowth => "Jackpot Window",
            EventKind::CommunityPot => "Community Pump",
        }
    }

    fn description(self) -> String {
        match self {
            EventKind::GrowthBonus => format!(
                "All /grow results get **+{GROWTH_BONUS_PERCENT}% growth** during this bonus window."
            ),
            EventKind::LowerCooldown => format!(
                "The /grow cooldown is lowered to **{LOWER_COOLDOWN_MINUTES} minutes** while this event is active."
            ),
            EventKind::LongerViagra => format!(
                "New /viagra activations last **{LONGER_VIAGRA_HOURS} hours** during this event."
            ),
            EventKind::DoubleGrowthRoll => {
                "Every /grow rolls twice and keeps the better result.".to_string()
            }
            EventKind::CompactGrowth => format!(
                "/grow becomes smaller but faster: **{}-{} cm** every **{COMPACT_GROWTH_COOLDOWN_MINUTES} minutes**.",
                COMPACT_GROWTH_RANGE.0, COMPACT_GROWTH_RANGE.1
            ),
            EventKind::JackpotGrowth => format!(
                "Every /grow has a **{}/{}** chance to hit an extra **+{JACKPOT_EXTRA_CM} cm** jackpot.",
                JACKPOT_CHANCE.0, JACKPOT_CHANCE.1
            ),
            EventKind::CommunityPot => format!(
                "Every /grow adds **+{COMMUNITY_POT_CM_PER_GROW} cm** to a global pot. When the event ends, the pot goes to a random participant."
            ),
        }
    }

    fn bonus_value(self) -> i64 {
        match self {
            EventKind::GrowthBonus => GROWTH_BONUS_PERCENT,
            EventKind::LowerCooldown => LOWER_COOLDOWN_MINUTES,
            EventKind::LongerViagra => LONGER_VIAGRA_HOURS,
            EventKind::DoubleGrowthRoll => 2,
            EventKind::CompactGrowth => COMPACT_GROWTH_COOLDOWN_MINUTES,
            EventKind::JackpotGrowth => JACKPOT_EXTRA_CM,
            EventKind::CommunityPot => COMMUNITY_POT_CM_PER_GROW,
        }
    }
}

#[derive(Clone, Debug)]
pub struct GlobalEvent {
    pub id: i64,
    pub kind: EventKind,
    pub name: String,
    pub description: String,
    pub bonus_value: i64,
    pub ends_at: NaiveDateTime,
}

impl GlobalEvent {
    pub fn growth_bonus_percent(&self) -> Option<i64> {
        (self.kind == EventKind::GrowthBonus).then_some(self.bonus_value)
    }

    pub fn grow_cooldown_minutes(&self) -> Option<i64> {
        match self.kind {
            EventKind::LowerCooldown => Some(self.bonus_value),
            EventKind::CompactGrowth => Some(COMPACT_GROWTH_COOLDOWN_MINUTES),
            _ => None,
        }
    }

    pub fn viagra_duration_hours(&self) -> Option<i64> {
        (self.kind == EventKind::LongerViagra).then_some(self.bonus_value)
    }

    pub fn growth_range(&self) -> Option<(i64, i64)> {
        (self.kind == EventKind::CompactGrowth).then_some(COMPACT_GROWTH_RANGE)
    }

    pub fn rolls_growth_twice(&self) -> bool {
        self.kind == EventKind::DoubleGrowthRoll
    }

    pub fn roll_jackpot(&self) -> Option<i64> {
        (self.kind == EventKind::JackpotGrowth
            && rand::rng().random_ratio(JACKPOT_CHANCE.0, JACKPOT_CHANCE.1))
        .then_some(JACKPOT_EXTRA_CM)
    }

    pub fn community_pot_cm_per_grow(&self) -> Option<i64> {
        (self.kind == EventKind::CommunityPot).then_some(COMMUNITY_POT_CM_PER_GROW)
    }
}

pub fn register() -> CreateCommand {
    CreateCommand::new("event").description("View the current global growth event")
}

pub async fn run(cmd: &Cmd<'_>) -> CommandResult {
    let response = match active_event(cmd.bot).await? {
        Some(event) => embed(
            format!("🌍 Global Event: {}", event.name),
            format!(
                "{}\n\nEnds {}",
                event.description,
                time::relative(event.ends_at)
            ),
            colors::GOLD,
        )
        .footer(CreateEmbedFooter::new(
            "Events are global and affect every server.",
        )),
        None => {
            let next_roll = next_period_start(Utc::now()).naive_utc();
            let possible = EVENT_WEIGHTS
                .iter()
                .map(|(kind, _)| kind.name())
                .collect::<Vec<_>>()
                .join(" · ");
            embed(
                "🌍 No Global Event",
                format!(
                    "Nothing special is happening right now.\n\nNext event roll {} ({}% chance).\n\n**Possible events:** {possible}",
                    time::relative(next_roll),
                    ACTIVATION_CHANCE.0 * 100 / ACTIVATION_CHANCE.1
                ),
                colors::NEUTRAL,
            )
            .footer(CreateEmbedFooter::new(
                "Events are global and affect every server.",
            ))
        }
    };
    cmd.reply(response).await
}

pub async fn active_event(bot: &Bot) -> sqlx::Result<Option<GlobalEvent>> {
    let Some(row) = sqlx::query!(
        r#"SELECT id as "id!", event_type, name, description, bonus_value, ends_at
         FROM global_events
         WHERE ends_at > datetime('now')
         ORDER BY ends_at DESC
         LIMIT 1"#
    )
    .fetch_optional(&bot.db)
    .await?
    else {
        return Ok(None);
    };

    let (Some(kind), Some(ends_at)) = (
        EventKind::from_str(&row.event_type),
        time::parse(&row.ends_at),
    ) else {
        warn!("Ignoring malformed global event {}", row.id);
        return Ok(None);
    };

    Ok(Some(GlobalEvent {
        id: row.id,
        kind,
        name: row.name,
        description: row.description,
        bonus_value: row.bonus_value,
        ends_at,
    }))
}

pub async fn add_to_community_pot(bot: &Bot, event_id: i64, amount: i64) -> sqlx::Result<()> {
    sqlx::query!(
        "UPDATE global_events
         SET pot_amount = pot_amount + ?
         WHERE id = ? AND event_type = 'community_pot' AND ends_at > datetime('now')",
        amount,
        event_id
    )
    .execute(&bot.db)
    .await?;
    Ok(())
}

fn period_start(now: DateTime<Utc>) -> DateTime<Utc> {
    let timestamp = now.timestamp();
    let start = timestamp - timestamp.rem_euclid(EVENT_PERIOD_HOURS * 3600);
    DateTime::from_timestamp(start, 0).expect("period start is a valid timestamp")
}

fn next_period_start(now: DateTime<Utc>) -> DateTime<Utc> {
    period_start(now) + Duration::hours(EVENT_PERIOD_HOURS)
}

/// Rolls a new event at every period boundary and pays out finished community pots.
pub async fn run_scheduler(ctx: Context, bot: Arc<Bot>) {
    loop {
        let now = Utc::now();
        let until_boundary = (next_period_start(now) - now).to_std().unwrap_or_default();
        tokio::time::sleep(until_boundary + TICK_DELAY).await;

        loop {
            match resolve_expired_community_pot(&bot).await {
                Ok(Some(message)) => info!("Event system: {message}"),
                Ok(None) => break,
                Err(why) => {
                    error!("Error resolving community pot: {why}");
                    break;
                }
            }
        }

        match try_start_event(&bot).await {
            Ok(Some(event)) => info!("Event system: started {}", event.name),
            Ok(None) => info!("Event system: no event this period"),
            Err(why) => error!("Error starting global event: {why}"),
        }

        update_presence(&ctx, &bot).await;
    }
}

async fn try_start_event(bot: &Bot) -> sqlx::Result<Option<GlobalEvent>> {
    if active_event(bot).await?.is_some()
        || !rand::rng().random_ratio(ACTIVATION_CHANCE.0, ACTIVATION_CHANCE.1)
    {
        return Ok(None);
    }

    let (kind, _) = *EVENT_WEIGHTS
        .choose_weighted(&mut rand::rng(), |&(_, weight)| weight)
        .expect("event weights are valid");
    let now = Utc::now();
    let started_at = time::format(now.naive_utc());
    let ends_at = next_period_start(now).naive_utc();
    let ends_at_str = time::format(ends_at);
    let (kind_str, name, description, bonus_value) = (
        kind.as_str(),
        kind.name(),
        kind.description(),
        kind.bonus_value(),
    );

    let id = sqlx::query!(
        "INSERT INTO global_events (event_type, name, description, bonus_value, started_at, ends_at)
         VALUES (?, ?, ?, ?, ?, ?)",
        kind_str,
        name,
        description,
        bonus_value,
        started_at,
        ends_at_str
    )
    .execute(&bot.db)
    .await?
    .last_insert_rowid();

    Ok(Some(GlobalEvent {
        id,
        kind,
        name: name.to_string(),
        description,
        bonus_value,
        ends_at,
    }))
}

async fn resolve_expired_community_pot(bot: &Bot) -> sqlx::Result<Option<String>> {
    let Some(event) = sqlx::query!(
        r#"SELECT id as "id!", name, pot_amount, started_at, ends_at
         FROM global_events
         WHERE event_type = 'community_pot'
           AND ends_at <= datetime('now')
           AND resolved_at IS NULL
         ORDER BY ends_at ASC
         LIMIT 1"#
    )
    .fetch_optional(&bot.db)
    .await?
    else {
        return Ok(None);
    };

    let grow = History::Grow.as_str();
    let participants = sqlx::query!(
        "SELECT DISTINCT user_id, guild_id
         FROM length_history
         WHERE growth_type = ? AND timestamp >= ? AND timestamp < ?",
        grow,
        event.started_at,
        event.ends_at
    )
    .fetch_all(&bot.db)
    .await?;

    let mut tx = bot.db.begin().await?;
    sqlx::query!(
        "UPDATE global_events SET resolved_at = datetime('now') WHERE id = ?",
        event.id
    )
    .execute(&mut *tx)
    .await?;

    let winner = participants.choose(&mut rand::rng());
    let message = match winner {
        _ if event.pot_amount <= 0 => {
            format!("{} ended, but nobody built up the pot", event.name)
        }
        None => format!(
            "{} ended with {} cm in the pot, but there were no eligible growers",
            event.name, event.pot_amount
        ),
        Some(winner) => {
            let new_length = sqlx::query_scalar!(
                "UPDATE dicks SET length = length + ? WHERE user_id = ? AND guild_id = ?
                 RETURNING length",
                event.pot_amount,
                winner.user_id,
                winner.guild_id
            )
            .fetch_one(&mut *tx)
            .await?;
            db::log_history(
                &mut *tx,
                &winner.user_id,
                &winner.guild_id,
                new_length,
                event.pot_amount,
                History::CommunityPot,
            )
            .await?;
            format!(
                "{} ended; user {} in guild {} won the {} cm pot (now {} cm)",
                event.name, winner.user_id, winner.guild_id, event.pot_amount, new_length
            )
        }
    };

    tx.commit().await?;
    Ok(Some(message))
}

pub async fn update_presence(ctx: &Context, bot: &Bot) {
    let status = match active_event(bot).await {
        Ok(Some(event)) => format!("🌍 Event: {} — /event", event.name),
        Ok(None) => "🍆 /grow your legacy".to_string(),
        Err(why) => {
            error!("Error fetching event for presence: {why}");
            return;
        }
    };
    ctx.set_activity(Some(ActivityData::custom(status)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn periods_align_to_utc_boundaries() {
        let now = DateTime::parse_from_rfc3339("2026-05-01T13:37:42Z")
            .unwrap()
            .to_utc();
        assert_eq!(period_start(now).to_rfc3339(), "2026-05-01T12:00:00+00:00");
        assert_eq!(
            next_period_start(now).to_rfc3339(),
            "2026-05-01T16:00:00+00:00"
        );
        let boundary = period_start(now);
        assert_eq!(period_start(boundary), boundary);
    }

    #[test]
    fn event_kinds_round_trip() {
        for (kind, _) in EVENT_WEIGHTS {
            assert_eq!(EventKind::from_str(kind.as_str()), Some(kind));
        }
    }
}
