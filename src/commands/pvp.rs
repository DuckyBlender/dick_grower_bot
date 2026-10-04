use crate::Bot;
use crate::commands::{Cmd, CommandResult};
use crate::db::{self, History};
use crate::time;
use crate::utils::{colors, embed};
use chrono::Duration;
use rand::RngExt;
use serenity::all::{
    ButtonStyle, CommandOptionType, ComponentInteraction, Context, CreateActionRow, CreateButton,
    CreateCommand, CreateCommandOption, CreateEmbed, CreateEmbedFooter, CreateInteractionResponse,
    CreateInteractionResponseMessage, GuildId, Mention, Mentionable, UserId,
};
use std::time::Instant;

const CHALLENGE_TTL_HOURS: i64 = 24;
pub const ACCEPT_PREFIX: &str = "pvp_accept:";
pub const CANCEL_PREFIX: &str = "pvp_cancel:";

pub struct PvpChallenge {
    challenger: UserId,
    guild_id: GuildId,
    bet: i64,
    created_at: Instant,
}

impl PvpChallenge {
    fn is_expired(&self) -> bool {
        self.created_at.elapsed().as_secs() >= (CHALLENGE_TTL_HOURS * 3600) as u64
    }
}

enum Claim {
    Missing,
    OwnChallenge,
    NotYours,
    Claimed(PvpChallenge),
}

pub fn register() -> CreateCommand {
    CreateCommand::new("pvp")
        .description("Start a dick battle")
        .add_option(
            CreateCommandOption::new(
                CommandOptionType::Integer,
                "bet",
                "The amount of cm you want to bet",
            )
            .required(true)
            .min_int_value(1),
        )
}

pub async fn run(cmd: &Cmd<'_>) -> CommandResult {
    let bet = cmd
        .interaction
        .data
        .options
        .first()
        .and_then(|option| option.value.as_i64())
        .unwrap_or_default();
    if bet < 1 {
        return cmd
            .reply_ephemeral(embed(
                "❌ Invalid Bet",
                "You need to bet at least 1 cm! Don't be so stingy with your centimeters.",
                colors::ERROR,
            ))
            .await;
    }

    db::ensure_user(&cmd.bot.db, &cmd.user, &cmd.guild).await?;
    let length = db::length(&cmd.bot.db, &cmd.user, &cmd.guild).await?;
    if length < bet {
        return cmd
            .reply_ephemeral(embed(
                "❌ Insufficient Length",
                format!(
                    "You only have **{length} cm** but you're trying to bet **{bet} cm**!\n\nYou can't bet what you don't have, buddy. Your ambition outweighs your equipment."
                ),
                colors::ERROR,
            ))
            .await;
    }

    let challenger = cmd.interaction.user.id;
    let challenge_id = cmd.interaction.id.get();
    {
        // One open challenge per user: a new one replaces the old one, whose buttons stop working.
        let mut challenges = cmd.bot.pvp_challenges.write().await;
        challenges
            .retain(|_, challenge| challenge.challenger != challenger && !challenge.is_expired());
        challenges.insert(
            challenge_id,
            PvpChallenge {
                challenger,
                guild_id: cmd.guild_id,
                bet,
                created_at: Instant::now(),
            },
        );
    }

    let flavor = match bet {
        100.. => "🤯 **LEGENDARY BET!** This is a high-stakes dick measuring contest!",
        50.. => "💰 **MASSIVE BET!** This is a high-stakes dick measuring contest!",
        25.. => "💰 That's quite a sizeable wager! Someone's feeling confident!",
        10.. => "A decent bet! More than a day's growth on the line.",
        5.. => "A reasonable bet for a friendly competition.",
        _ => "A cautious bet. Not everyone's ready to risk their precious centimeters!",
    };
    let expires = time::now() + Duration::hours(CHALLENGE_TTL_HOURS);
    let buttons = CreateActionRow::Buttons(vec![
        CreateButton::new(format!("{ACCEPT_PREFIX}{challenge_id}"))
            .label("Accept Challenge")
            .style(ButtonStyle::Success)
            .emoji('🔥'),
        CreateButton::new(format!("{CANCEL_PREFIX}{challenge_id}"))
            .label("Cancel")
            .style(ButtonStyle::Secondary),
    ]);

    cmd.respond(
        CreateInteractionResponseMessage::new()
            .embed(
                embed(
                    "🥊 Dick Battle!",
                    format!(
                        "{} has started a dick battle!\n\nBet: **{bet} cm**\n{flavor}\n\nExpires {}",
                        challenger.mention(),
                        time::relative(expires)
                    ),
                    colors::INFO,
                )
                .footer(CreateEmbedFooter::new(
                    "Anyone can accept this challenge. Both players roll 1-100, highest roll takes the bet!",
                )),
            )
            .components(vec![buttons]),
    )
    .await
}

pub async fn handle_component(
    ctx: &Context,
    bot: &Bot,
    component: &ComponentInteraction,
) -> CommandResult {
    let custom_id = component.data.custom_id.as_str();
    let (accept, raw_id) = if let Some(id) = custom_id.strip_prefix(ACCEPT_PREFIX) {
        (true, id)
    } else if let Some(id) = custom_id.strip_prefix(CANCEL_PREFIX) {
        (false, id)
    } else {
        return Ok(());
    };
    let challenge_id = raw_id.parse::<u64>().unwrap_or_default();
    let user = component.user.id;

    let claim = {
        let mut challenges = bot.pvp_challenges.write().await;
        match challenges.get(&challenge_id) {
            None => Claim::Missing,
            Some(challenge) if accept && challenge.challenger == user => Claim::OwnChallenge,
            Some(challenge) if !accept && challenge.challenger != user => Claim::NotYours,
            Some(_) => Claim::Claimed(
                challenges
                    .remove(&challenge_id)
                    .expect("challenge is present"),
            ),
        }
    };

    let challenge = match claim {
        Claim::Missing => {
            return reply_ephemeral(
                ctx,
                component,
                embed(
                    "❓ No Active Challenge",
                    "This challenge no longer exists. It might have expired, been cancelled, or been accepted by someone else.",
                    colors::NEUTRAL,
                ),
            )
            .await;
        }
        Claim::OwnChallenge => {
            return reply_ephemeral(
                ctx,
                component,
                embed(
                    "🤨 Self-Challenge Detected",
                    "You can't accept your own challenge! That would be... weird.",
                    colors::WARNING,
                ),
            )
            .await;
        }
        Claim::NotYours => {
            return reply_ephemeral(
                ctx,
                component,
                embed(
                    "🤨 Not Your Battle",
                    "Only the challenger can cancel this battle.",
                    colors::WARNING,
                ),
            )
            .await;
        }
        Claim::Claimed(challenge) => challenge,
    };

    let bet = challenge.bet;
    let challenger = challenge.challenger;
    if !accept {
        return close_message(
            ctx,
            component,
            embed(
                "🏳️ Battle Cancelled",
                format!(
                    "{} chickened out of their **{bet} cm** dick battle.",
                    challenger.mention()
                ),
                colors::NEUTRAL,
            ),
        )
        .await;
    }
    if challenge.is_expired() {
        return close_message(
            ctx,
            component,
            embed(
                "⏰ Challenge Expired",
                format!("This challenge expired after {CHALLENGE_TTL_HOURS} hours. Nobody was brave enough!"),
                colors::NEUTRAL,
            ),
        )
        .await;
    }

    let guild = challenge.guild_id.to_string();
    let challenger_str = challenger.to_string();
    let acceptor_str = user.to_string();
    db::ensure_user(&bot.db, &acceptor_str, &guild).await?;

    let challenger_length = db::length(&bot.db, &challenger_str, &guild).await?;
    if challenger_length < bet {
        return close_message(
            ctx,
            component,
            embed(
                "❌ Battle Cancelled",
                format!(
                    "{} only has **{challenger_length} cm** left and can't cover the **{bet} cm** bet anymore.",
                    challenger.mention()
                ),
                colors::ERROR,
            ),
        )
        .await;
    }

    let acceptor_length = db::length(&bot.db, &acceptor_str, &guild).await?;
    if acceptor_length < bet {
        // Leave the challenge open for someone who can afford it.
        bot.pvp_challenges
            .write()
            .await
            .insert(challenge_id, challenge);
        return reply_ephemeral(
            ctx,
            component,
            embed(
                "❌ Insufficient Length",
                format!(
                    "You only have **{acceptor_length} cm** but this battle needs **{bet} cm**!\n\nYou can't compete with what you don't have. Grow a bit more first."
                ),
                colors::ERROR,
            ),
        )
        .await;
    }

    let (challenger_roll, acceptor_roll) = {
        let mut rng = rand::rng();
        (rng.random_range(1..=100_i64), rng.random_range(1..=100_i64))
    };

    if challenger_roll == acceptor_roll {
        let comment = match bet {
            30.. => {
                format!("A {bet} cm bet and it ends in a tie?! The dick gods must be laughing!")
            }
            15.. => "Insanity! Neither dick emerged victorious today!".to_string(),
            _ => "What are the odds?! Both measuring exactly the same!".to_string(),
        };
        return close_message(
            ctx,
            component,
            embed(
                "🤯 INCREDIBLE! It's a Tie!",
                format!(
                    "{} rolled **{challenger_roll}**\n{} rolled **{acceptor_roll}**\n\n{comment}\n\nNo winners, no losers: everyone keeps their centimeters.",
                    challenger.mention(),
                    user.mention()
                ),
                colors::PURPLE,
            )
            .footer(CreateEmbedFooter::new(
                "A moment that will go down in dick-measuring history!",
            )),
        )
        .await;
    }

    let (winner, loser, winner_roll, loser_roll) = if challenger_roll > acceptor_roll {
        (challenger, user, challenger_roll, acceptor_roll)
    } else {
        (user, challenger, acceptor_roll, challenger_roll)
    };
    let (winner_str, loser_str) = (winner.to_string(), loser.to_string());

    // Both balances are re-checked inside the transaction so concurrent gifts or battles
    // can't push anyone below zero.
    let mut tx = bot.db.begin().await?;
    let loser_length = sqlx::query_scalar!(
        "UPDATE dicks
         SET length = length - ?, pvp_losses = pvp_losses + 1, pvp_current_streak = 0,
             cm_lost = cm_lost + ?
         WHERE user_id = ? AND guild_id = ? AND length >= ?
         RETURNING length",
        bet,
        bet,
        loser_str,
        guild,
        bet
    )
    .fetch_optional(&mut *tx)
    .await?;
    let winner_row = sqlx::query!(
        "UPDATE dicks
         SET length = length + ?, pvp_wins = pvp_wins + 1,
             pvp_current_streak = pvp_current_streak + 1,
             pvp_max_streak = MAX(pvp_max_streak, pvp_current_streak + 1),
             cm_won = cm_won + ?
         WHERE user_id = ? AND guild_id = ? AND length >= ?
         RETURNING length, pvp_current_streak",
        bet,
        bet,
        winner_str,
        guild,
        bet
    )
    .fetch_optional(&mut *tx)
    .await?;

    let (Some(loser_length), Some(winner_row)) = (loser_length, winner_row) else {
        return close_message(
            ctx,
            component,
            embed(
                "❌ Battle Cancelled",
                "Someone's length changed mid-battle and they can't cover the bet anymore.",
                colors::ERROR,
            ),
        )
        .await;
    };
    db::log_history(
        &mut *tx,
        &winner_str,
        &guild,
        winner_row.length,
        bet,
        History::PvpWon,
    )
    .await?;
    db::log_history(
        &mut *tx,
        &loser_str,
        &guild,
        loser_length,
        -bet,
        History::PvpLost,
    )
    .await?;
    tx.commit().await?;

    let (winner_mention, loser_mention) = (winner.mention(), loser.mention());
    let streak = winner_row.pvp_current_streak;
    let streak_comment = match streak {
        5.. => format!(
            "\n🔥 {winner_mention} is on a **{streak}-win streak**! Absolutely dominating! 👑"
        ),
        3.. => format!("\n🔥 {winner_mention} is on a **{streak}-win streak**! 📈"),
        _ => String::new(),
    };

    close_message(
        ctx,
        component,
        embed(
            "🏆 Dick Battle Results!",
            format!(
                "👑 {winner_mention} won **{bet} cm** from {loser_mention}!{streak_comment}\n\n{}",
                taunt(winner_roll - loser_roll, bet, winner_mention, loser_mention)
            ),
            colors::SUCCESS,
        )
        .field(
            "🎲 Rolls",
            format!("👑 {winner_mention}: **{winner_roll}**\n{loser_mention}: **{loser_roll}**"),
            true,
        )
        .field(
            "📏 New Lengths",
            format!(
                "👑 {winner_mention}: **{} cm**\n{loser_mention}: **{loser_length} cm**",
                winner_row.length
            ),
            true,
        )
        .footer(CreateEmbedFooter::new("Size DOES matter after all!")),
    )
    .await
}

fn taunt(margin: i64, bet: i64, winner: Mention, loser: Mention) -> String {
    match margin {
        51.. if bet >= 30 => format!(
            "💀 It wasn't even close! {winner}'s dick absolutely DEMOLISHED {loser}'s in a historic beatdown! Those {bet} centimeters will be remembered for generations! 📜"
        ),
        51.. => format!(
            "💀 It wasn't even close! {winner}'s dick destroyed {loser}'s in an absolute massacre! ⚰️"
        ),
        21.. if bet >= 20 => format!(
            "🏆 {winner}'s dick clearly outclassed {loser}'s in this epic showdown! That's {bet} cm of pride changing hands!"
        ),
        21.. => format!("🏆 {winner}'s dick clearly outclassed {loser}'s in this epic showdown!"),
        6.. if bet >= 15 => format!(
            "🥇 A close match, but {winner}'s dick had just enough extra length to claim victory and snatch those {bet} valuable centimeters!"
        ),
        6.. => format!(
            "🥇 A close match, but {winner}'s dick had just enough extra length to claim victory!"
        ),
        _ if bet >= 25 => format!(
            "😱 WHAT A NAIL-BITER! {winner}'s dick barely edged out {loser}'s by a hair's width! Those {bet} centimeters were almost too close to call!"
        ),
        _ => format!(
            "😮 That was incredibly close! {winner}'s dick barely edged out {loser}'s by a hair's width!"
        ),
    }
}

async fn reply_ephemeral(
    ctx: &Context,
    component: &ComponentInteraction,
    embed: CreateEmbed,
) -> CommandResult {
    component
        .create_response(
            &ctx.http,
            CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .embed(embed)
                    .ephemeral(true),
            ),
        )
        .await?;
    Ok(())
}

/// Replaces the challenge message with a final result and removes its buttons.
async fn close_message(
    ctx: &Context,
    component: &ComponentInteraction,
    embed: CreateEmbed,
) -> CommandResult {
    component
        .create_response(
            &ctx.http,
            CreateInteractionResponse::UpdateMessage(
                CreateInteractionResponseMessage::new()
                    .embed(embed)
                    .components(vec![]),
            ),
        )
        .await?;
    Ok(())
}
