use crate::Bot;
use chrono::{Duration, NaiveDateTime};
use log::error;
use rand::RngExt;
use serenity::all::{
    CommandInteraction, CreateEmbed, CreateEmbedFooter, CreateInteractionResponse,
    CreateInteractionResponseMessage,
};
use serenity::prelude::*;
use sqlx::Row;

pub const EVENT_DURATION_HOURS: i64 = 4;
const EVENT_ACTIVATION_CHANCE_NUMERATOR: u32 = 1;
const EVENT_ACTIVATION_CHANCE_DENOMINATOR: u32 = 2;

const GROWTH_BONUS_EVENT_WEIGHT: u32 = 1;
const LOWER_COOLDOWN_EVENT_WEIGHT: u32 = 1;
const LONGER_VIAGRA_EVENT_WEIGHT: u32 = 1;
const DOUBLE_GROWTH_ROLL_EVENT_WEIGHT: u32 = 1;
const COMPACT_GROWTH_EVENT_WEIGHT: u32 = 1;
const JACKPOT_GROWTH_EVENT_WEIGHT: u32 = 1;
const COMMUNITY_POT_EVENT_WEIGHT: u32 = 1;

const GROWTH_BONUS_PERCENT: i64 = 25;
const LOWER_COOLDOWN_MINUTES: i64 = 30;
const LONGER_VIAGRA_HOURS: i64 = 12;
const COMPACT_GROWTH_MIN_CM: i64 = 1;
const COMPACT_GROWTH_MAX_CM: i64 = 5;
const COMPACT_GROWTH_COOLDOWN_MINUTES: i64 = 15;
const JACKPOT_EXTRA_CM: i64 = 25;
const JACKPOT_CHANCE_NUMERATOR: u32 = 1;
const JACKPOT_CHANCE_DENOMINATOR: u32 = 10;
const COMMUNITY_POT_CM_PER_GROW: i64 = 1;

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
        match value {
            "growth_bonus" => Some(EventKind::GrowthBonus),
            "lower_cooldown" => Some(EventKind::LowerCooldown),
            "longer_viagra" => Some(EventKind::LongerViagra),
            "double_growth_roll" => Some(EventKind::DoubleGrowthRoll),
            "compact_growth" => Some(EventKind::CompactGrowth),
            "jackpot_growth" => Some(EventKind::JackpotGrowth),
            "community_pot" => Some(EventKind::CommunityPot),
            _ => None,
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
    pub fn growth_multiplier(&self) -> Option<f64> {
        (self.kind == EventKind::GrowthBonus).then_some(1.0 + self.bonus_value as f64 / 100.0)
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
        (self.kind == EventKind::CompactGrowth)
            .then_some((COMPACT_GROWTH_MIN_CM, COMPACT_GROWTH_MAX_CM))
    }

    pub fn rolls_growth_twice(&self) -> bool {
        matches!(self.kind, EventKind::DoubleGrowthRoll)
    }

    pub fn jackpot_extra_cm(&self) -> Option<i64> {
        if self.kind == EventKind::JackpotGrowth
            && rand::rng().random_ratio(JACKPOT_CHANCE_NUMERATOR, JACKPOT_CHANCE_DENOMINATOR)
        {
            Some(JACKPOT_EXTRA_CM)
        } else {
            None
        }
    }

    pub fn community_pot_cm_per_grow(&self) -> Option<i64> {
        (self.kind == EventKind::CommunityPot).then_some(COMMUNITY_POT_CM_PER_GROW)
    }

    pub fn ends_discord_timestamp(&self) -> String {
        format!("<t:{}:R>", self.ends_at.and_utc().timestamp())
    }
}

pub async fn get_active_global_event(bot: &Bot) -> Option<GlobalEvent> {
    let now = chrono::Utc::now().naive_utc();

    let row = sqlx::query(
        "SELECT id, event_type, name, description, bonus_value, ends_at
         FROM global_events
         WHERE ends_at > datetime('now')
         ORDER BY ends_at DESC
         LIMIT 1",
    )
    .fetch_optional(&bot.database)
    .await
    .ok()??;

    let event_type: String = row.try_get("event_type").ok()?;
    let kind = EventKind::from_str(&event_type)?;
    let ends_at_str: String = row.try_get("ends_at").ok()?;
    let ends_at = NaiveDateTime::parse_from_str(&ends_at_str, "%Y-%m-%d %H:%M:%S").ok()?;

    if ends_at <= now {
        return None;
    }

    Some(GlobalEvent {
        id: row.try_get("id").ok()?,
        kind,
        name: row.try_get("name").ok()?,
        description: row.try_get("description").ok()?,
        bonus_value: row.try_get("bonus_value").ok()?,
        ends_at,
    })
}

pub async fn handle_event_command(
    ctx: &Context,
    command: &CommandInteraction,
) -> Result<(), serenity::Error> {
    let data = ctx.data.read().await;
    let bot = data.get::<Bot>().unwrap();

    if let Some(event) = get_active_global_event(bot).await {
        let builder = CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new().add_embed(
                CreateEmbed::new()
                    .title(format!("🌍 Global Event Active: {}", event.name))
                    .description(format!(
                        "{}\n\nEvent ends: {}",
                        event.description,
                        event.ends_discord_timestamp()
                    ))
                    .color(0xF1C40F)
                    .footer(CreateEmbedFooter::new(
                        "This event is global and affects every server.",
                    )),
            ),
        );
        command.create_response(&ctx.http, builder).await
    } else {
        let builder = CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new().add_embed(
                CreateEmbed::new()
                    .title("🌍 No Global Event")
                    .description(
                        "Nothing special is happening globally right now. Events start automatically with a 50% chance every so often.",
                    )
                    .color(0xAAAAAA)
                    .footer(CreateEmbedFooter::new(
                        "Events are global and affect every server when active.",
                    )),
            ),
        );
        command.create_response(&ctx.http, builder).await
    }
}

pub fn roll_global_event() -> GlobalEvent {
    let growth_bonus_cutoff = GROWTH_BONUS_EVENT_WEIGHT;
    let lower_cooldown_cutoff = growth_bonus_cutoff + LOWER_COOLDOWN_EVENT_WEIGHT;
    let longer_viagra_cutoff = lower_cooldown_cutoff + LONGER_VIAGRA_EVENT_WEIGHT;
    let double_growth_cutoff = longer_viagra_cutoff + DOUBLE_GROWTH_ROLL_EVENT_WEIGHT;
    let compact_growth_cutoff = double_growth_cutoff + COMPACT_GROWTH_EVENT_WEIGHT;
    let jackpot_growth_cutoff = compact_growth_cutoff + JACKPOT_GROWTH_EVENT_WEIGHT;
    let total_event_weight = jackpot_growth_cutoff + COMMUNITY_POT_EVENT_WEIGHT;
    let roll = rand::rng().random_range(0..total_event_weight);

    if roll < growth_bonus_cutoff {
        GlobalEvent {
            id: 0,
            kind: EventKind::GrowthBonus,
            name: "Growth Surge".to_string(),
            description: format!(
                "All /grow results get **+{}% growth** during this bonus window.",
                GROWTH_BONUS_PERCENT
            ),
            bonus_value: GROWTH_BONUS_PERCENT,
            ends_at: chrono::Utc::now().naive_utc(),
        }
    } else if roll < lower_cooldown_cutoff {
        GlobalEvent {
            id: 0,
            kind: EventKind::LowerCooldown,
            name: "Fast Hands".to_string(),
            description: format!(
                "The /grow cooldown is lowered to **{} minutes** while this event is active.",
                LOWER_COOLDOWN_MINUTES
            ),
            bonus_value: LOWER_COOLDOWN_MINUTES,
            ends_at: chrono::Utc::now().naive_utc(),
        }
    } else if roll < longer_viagra_cutoff {
        GlobalEvent {
            id: 0,
            kind: EventKind::LongerViagra,
            name: "Extended Pharmacy Hours".to_string(),
            description: format!(
                "New /viagra activations last **{} hours** during this event.",
                LONGER_VIAGRA_HOURS
            ),
            bonus_value: LONGER_VIAGRA_HOURS,
            ends_at: chrono::Utc::now().naive_utc(),
        }
    } else if roll < double_growth_cutoff {
        GlobalEvent {
            id: 0,
            kind: EventKind::DoubleGrowthRoll,
            name: "Double Trouble".to_string(),
            description: "Every /grow rolls twice and keeps the better result.".to_string(),
            bonus_value: 2,
            ends_at: chrono::Utc::now().naive_utc(),
        }
    } else if roll < compact_growth_cutoff {
        GlobalEvent {
            id: 0,
            kind: EventKind::CompactGrowth,
            name: "Quick Sprouts".to_string(),
            description: format!(
                "/grow becomes smaller but faster: **{}-{} cm** every **{} minutes**.",
                COMPACT_GROWTH_MIN_CM, COMPACT_GROWTH_MAX_CM, COMPACT_GROWTH_COOLDOWN_MINUTES
            ),
            bonus_value: COMPACT_GROWTH_COOLDOWN_MINUTES,
            ends_at: chrono::Utc::now().naive_utc(),
        }
    } else if roll < jackpot_growth_cutoff {
        GlobalEvent {
            id: 0,
            kind: EventKind::JackpotGrowth,
            name: "Jackpot Window".to_string(),
            description: format!(
                "Every /grow has a **{}/{}** chance to hit an extra **+{} cm** jackpot.",
                JACKPOT_CHANCE_NUMERATOR, JACKPOT_CHANCE_DENOMINATOR, JACKPOT_EXTRA_CM
            ),
            bonus_value: JACKPOT_EXTRA_CM,
            ends_at: chrono::Utc::now().naive_utc(),
        }
    } else {
        GlobalEvent {
            id: 0,
            kind: EventKind::CommunityPot,
            name: "Community Pump".to_string(),
            description: format!(
                "Every /grow adds **+{} cm** to a global pot. The pot is automatically awarded to a random participant after the event ends.",
                COMMUNITY_POT_CM_PER_GROW
            ),
            bonus_value: COMMUNITY_POT_CM_PER_GROW,
            ends_at: chrono::Utc::now().naive_utc(),
        }
    }
}

pub async fn add_to_community_pot(bot: &Bot, event_id: i64, amount: i64) {
    if let Err(why) = sqlx::query(
        "UPDATE global_events
         SET pot_amount = pot_amount + ?
         WHERE id = ? AND event_type = 'community_pot' AND ends_at > datetime('now')",
    )
    .bind(amount)
    .bind(event_id)
    .execute(&bot.database)
    .await
    {
        error!("Error adding to community pot: {:?}", why);
    }
}

pub async fn resolve_expired_community_pot(bot: &Bot) -> Option<String> {
    let event = sqlx::query(
        "SELECT id, name, pot_amount, started_at, ends_at
         FROM global_events
         WHERE event_type = 'community_pot'
           AND ends_at <= datetime('now')
           AND resolved_at IS NULL
         ORDER BY ends_at ASC
         LIMIT 1",
    )
    .fetch_optional(&bot.database)
    .await
    .ok()??;

    let event_id = event.try_get::<i64, _>("id").ok()?;
    let event_name = event.try_get::<String, _>("name").ok()?;
    let pot_amount = event.try_get::<i64, _>("pot_amount").ok()?;
    let started_at = event.try_get::<String, _>("started_at").ok()?;
    let ends_at = event.try_get::<String, _>("ends_at").ok()?;

    let now_str = chrono::Utc::now()
        .naive_utc()
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();

    if pot_amount <= 0 {
        mark_community_pot_resolved(bot, event_id, &now_str).await;
        return Some(format!(
            "**{}** ended, but nobody built up the pot.",
            event_name
        ));
    }

    let participants = sqlx::query(
        "SELECT user_id, guild_id, length
         FROM dicks
         WHERE last_grow >= ? AND last_grow <= ?",
    )
    .bind(&started_at)
    .bind(&ends_at)
    .fetch_all(&bot.database)
    .await
    .ok()?;

    if participants.is_empty() {
        mark_community_pot_resolved(bot, event_id, &now_str).await;
        return Some(format!(
            "**{}** ended with **{} cm** in the pot, but there were no eligible growers.",
            event_name, pot_amount
        ));
    }

    let winner_idx = rand::rng().random_range(0..participants.len());
    let winner = &participants[winner_idx];
    let user_id = winner.try_get::<String, _>("user_id").ok()?;
    let guild_id = winner.try_get::<String, _>("guild_id").ok()?;
    let old_length = winner.try_get::<i64, _>("length").ok()?;
    let new_length = old_length + pot_amount;

    let mut tx = bot.database.begin().await.ok()?;

    if sqlx::query("UPDATE dicks SET length = length + ? WHERE user_id = ? AND guild_id = ?")
        .bind(pot_amount)
        .bind(&user_id)
        .bind(&guild_id)
        .execute(&mut *tx)
        .await
        .is_err()
    {
        return None;
    }

    if sqlx::query(
        "INSERT INTO length_history (user_id, guild_id, length, growth_amount, growth_type)
         VALUES (?, ?, ?, ?, 'community_pot')",
    )
    .bind(&user_id)
    .bind(&guild_id)
    .bind(new_length)
    .bind(pot_amount)
    .execute(&mut *tx)
    .await
    .is_err()
    {
        return None;
    }

    if sqlx::query("UPDATE global_events SET resolved_at = ? WHERE id = ?")
        .bind(&now_str)
        .bind(event_id)
        .execute(&mut *tx)
        .await
        .is_err()
    {
        return None;
    }

    tx.commit().await.ok()?;

    Some(format!(
        "**{}** ended with a **{} cm** pot.\n\nWinner: <@{}>\nNew length: **{} cm**",
        event_name, pot_amount, user_id, new_length
    ))
}

async fn mark_community_pot_resolved(bot: &Bot, event_id: i64, resolved_at: &str) {
    if let Err(why) = sqlx::query("UPDATE global_events SET resolved_at = ? WHERE id = ?")
        .bind(resolved_at)
        .bind(event_id)
        .execute(&bot.database)
        .await
    {
        error!("Error resolving empty community pot: {:?}", why);
    }
}

pub async fn try_start_new_event(bot: &Bot) -> Option<GlobalEvent> {
    if get_active_global_event(bot).await.is_some() {
        return None;
    }

    if !rand::rng().random_ratio(
        EVENT_ACTIVATION_CHANCE_NUMERATOR,
        EVENT_ACTIVATION_CHANCE_DENOMINATOR,
    ) {
        return None;
    }

    let event = roll_global_event();
    let now = chrono::Utc::now().naive_utc();
    let ends_at = now + Duration::hours(EVENT_DURATION_HOURS);
    let now_str = now.format("%Y-%m-%d %H:%M:%S").to_string();
    let ends_at_str = ends_at.format("%Y-%m-%d %H:%M:%S").to_string();

    if let Err(why) = sqlx::query(
        "INSERT INTO global_events (event_type, name, description, bonus_value, started_at, ends_at)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(event.kind.as_str())
    .bind(&event.name)
    .bind(&event.description)
    .bind(event.bonus_value)
    .bind(now_str)
    .bind(ends_at_str)
    .execute(&bot.database)
    .await
    {
        error!("Error creating global event: {:?}", why);
        return None;
    }

    Some(GlobalEvent { ends_at, ..event })
}

pub async fn tick_event_system(bot: &Bot) -> Vec<String> {
    let mut messages = Vec::new();

    if let Some(msg) = resolve_expired_community_pot(bot).await {
        messages.push(msg);
    }

    if let Some(event) = try_start_new_event(bot).await {
        messages.push(format!(
            "🌍 Global Event Started: {} — {} (ends {})",
            event.name,
            event.description,
            event.ends_discord_timestamp()
        ));
    }

    messages
}
