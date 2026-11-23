use crate::commands::{Cmd, CommandResult};
use crate::db;
use crate::time;
use crate::utils::{colors, embed};
use rand::seq::IndexedRandom;
use serenity::all::{CreateCommand, CreateEmbedFooter};

/// Chance in percent that the pill backfires and the grow roll is lost instead of doubled.
pub const BACKFIRE_PERCENT: u32 = 25;

/// Why the pill backfired, shown when a /grow shrinks the player.
const BACKFIRES: &[&str] = &[
    "Turns out you're allergic to the little blue pill. Your dick swelled... inward.",
    "That wasn't viagra. That was a Tic Tac from 2009.",
    "The pharmacy mixed up your order with someone's shrinking cream.",
    "You washed it down with grapefruit juice. The pharmacist warned you.",
    "It expired in 2014. Should have read the label.",
    "You bought it from a guy behind the gas station. Rookie mistake.",
    "Side effects may include: exactly this.",
    "Your dick saw the pill coming and got stage fright.",
    "It was a placebo, and your dick felt personally betrayed.",
    "You took it with three energy drinks. Your dick filed a complaint.",
    "Counterfeit pill from a pop-up ad. Your dick paid the price.",
    "Your body rejected the pill like a bad organ transplant.",
    "Mild allergic reaction: hives, sneezing and severe shrinkage.",
    "You swallowed the silica gel packet instead of the pill.",
];

pub fn backfire_reason() -> &'static str {
    BACKFIRES
        .choose(&mut rand::rng())
        .expect("backfire reasons are not empty")
}

pub fn register() -> CreateCommand {
    CreateCommand::new("viagra").description(format!(
        "Take today's pill: your next /grow is doubled, or a {BACKFIRE_PERCENT}% chance it backfires"
    ))
}

pub async fn run(cmd: &Cmd<'_>) -> CommandResult {
    let db = &cmd.bot.db;
    db::ensure_user(db, &cmd.user, &cmd.guild).await?;

    // Checking and taking happen in one statement so two invocations can't both get a pill.
    let taken = sqlx::query!(
        "UPDATE dicks SET viagra_pill_pending = 1, viagra_last_used = datetime('now')
         WHERE user_id = ? AND guild_id = ? AND viagra_pill_pending = 0
           AND (viagra_last_used IS NULL OR date(viagra_last_used) < date('now'))",
        cmd.user,
        cmd.guild
    )
    .execute(db)
    .await?
    .rows_affected()
        > 0;

    if !taken {
        let pending = sqlx::query_scalar!(
            "SELECT viagra_pill_pending FROM dicks WHERE user_id = ? AND guild_id = ?",
            cmd.user,
            cmd.guild
        )
        .fetch_one(db)
        .await?
            > 0;
        let description = if pending {
            "You already took a pill and it's still waiting for your next /grow.\n\nGo use it before you start doubling up."
                .to_string()
        } else {
            format!(
                "One pill per day, champ. Your body is still processing the last one.\n\nNext pill available {}.",
                time::relative(time::next_utc_midnight())
            )
        };
        return cmd
            .reply_ephemeral(
                embed("🚫 Pharmacy Closed", description, colors::WARNING).footer(
                    CreateEmbedFooter::new("Too much enhancement can lead to... complications."),
                ),
            )
            .await;
    }

    cmd.reply(
        embed(
            "💊 Pill Swallowed!",
            format!(
                "The little blue pill is in your system and waiting for your next /grow.\n\n\
                 • **{}%** chance: that grow is **doubled**\n\
                 • **{BACKFIRE_PERCENT}%** chance: side effects, and you **shrink** by the roll instead\n\n\
                 No rush, it won't wear off.",
                100 - BACKFIRE_PERCENT
            ),
            colors::INFO,
        )
        .footer(CreateEmbedFooter::new(
            "Consult your doctor if your dick lasts longer than 4 hours.",
        )),
    )
    .await
}
