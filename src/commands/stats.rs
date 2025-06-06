use crate::Bot;
use crate::commands::escape_markdown;
use crate::commands::viagra::get_viagra_status;
use crate::time::check_cooldown_minutes;
use crate::utils::{get_fun_title_by_rank, ordinal_suffix};
use chrono::NaiveDateTime;
use log::error;
use serenity::all::{
    CommandInteraction, CreateEmbed, CreateEmbedFooter, CreateInteractionResponse,
    CreateInteractionResponseMessage,
};
use serenity::prelude::*;

pub async fn handle_stats_command(
    ctx: &Context,
    command: &CommandInteraction,
) -> Result<(), serenity::Error> {
    let data = ctx.data.read().await;
    let bot = data.get::<Bot>().unwrap();

    // Check if a user was specified
    let target_user = if let Some(option) = command.data.options.first() {
        match option.value.as_user_id() {
            Some(user_id) => {
                match user_id.to_user(ctx).await {
                    Ok(user) => user,
                    Err(_) => {
                        // Could not fetch user, fallback to command user
                        command.user.clone()
                    }
                }
            }
            None => command.user.clone(),
        }
    } else {
        command.user.clone()
    };

    let is_self = target_user.id == command.user.id;
    let user_id = target_user.id.to_string();
    let guild_id = command.guild_id.unwrap().to_string();

    // Get user stats
    let user_stats = match sqlx::query!(
        "SELECT length, dick_of_day_count, last_grow, 
                pvp_wins, pvp_losses, pvp_max_streak, pvp_current_streak,
                cm_won, cm_lost
         FROM dicks 
         WHERE user_id = ? AND guild_id = ?",
        user_id,
        guild_id
    )
    .fetch_optional(&bot.database)
    .await
    {
        Ok(Some(stats)) => stats,
        Ok(None) => {
            let msg = if is_self {
                "You haven't started growing your dick yet! Use /grow to begin your journey to greatness."
            } else {
                "This user hasn't started growing their dick yet!"
            };

            let builder = CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().add_embed(
                    CreateEmbed::new()
                        .title("❓ No Stats Found")
                        .description(msg)
                        .color(0xAAAAAA),
                ),
            );
            return command.create_response(&ctx.http, builder).await;
        }
        Err(why) => {
            error!("Database error: {:?}", why);
            let builder = CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .add_embed(
                        CreateEmbed::new()
                            .title("⚠️ Database Error")
                            .description("Failed to retrieve the stats. The server's ruler broke.")
                            .color(0xFF0000),
                    )
                    .ephemeral(true),
            );
            return command.create_response(&ctx.http, builder).await;
        }
    };

    // Get rank
    let rank = match sqlx::query!(
        "SELECT COUNT(*) as rank FROM dicks 
         WHERE guild_id = ? AND length > (
            SELECT length FROM dicks WHERE user_id = ? AND guild_id = ?
         )",
        guild_id,
        user_id,
        guild_id
    )
    .fetch_one(&bot.database)
    .await
    {
        Ok(record) => record.rank + 1, // +1 because we're counting users with MORE length
        Err(why) => {
            error!("Error fetching rank: {:?}", why);
            0
        }
    };
    let rank = rank as usize; // Safe to cast to usize

    // Calculate growth status - only show for own user
    let last_grow = NaiveDateTime::parse_from_str(&user_stats.last_grow, "%Y-%m-%d %H:%M:%S")
        .unwrap_or_default();

    // Check if user can grow today
    let time_left = check_cooldown_minutes(&last_grow);
    let unix_timestamp = chrono::Utc::now().timestamp() + time_left.num_seconds();
    let discord_timestamp = format!("<t:{}:R>", unix_timestamp);

    let growth_status = if is_self {
        if time_left.is_zero() {
            "✅ You can grow now! Use /grow".to_string()
        } else {
            format!("⏰ Next growth in: {discord_timestamp}",)
        }
    } else if time_left.is_zero() {
        "✅ Can grow now".to_string()
    } else {
        "⏰ Already grew today".to_string()
    };

    // Get viagra status
    let (viagra_active, effect_ends, next_available) = get_viagra_status(bot, &user_id, &guild_id).await;
    
    let viagra_status = if viagra_active {
        if let Some(ends) = effect_ends {
            format!("💊 **ACTIVE** (ends {})", ends)
        } else {
            "💊 **ACTIVE**".to_string()
        }
    } else if let Some(available) = next_available {
        format!("💊 Available {}", available)
    } else {
        "💊 Available now".to_string()
    };

    // Calculate win rate
    let total_fights = user_stats.pvp_wins + user_stats.pvp_losses;
    let win_rate = if total_fights > 0 {
        (user_stats.pvp_wins as f64 / total_fights as f64) * 100.0
    } else {
        0.0
    };

    // Funny comment based on length
    let fun_title = get_fun_title_by_rank(rank);
    let length_comment = if user_stats.length <= 0 {
        if is_self {
            "Your dick is practically an innie at this point. Keep trying!"
        } else {
            "Their dick is practically an innie at this point. Tragic!"
        }
    } else if user_stats.length < 50 {
        "It's... cute? At least that's what they'll say to be nice."
    } else if user_stats.length < 100 {
        "Not bad! In the average zone. But who wants to be average?"
    } else if user_stats.length < 150 {
        "Impressive length! That's some serious heat down there."
    } else if user_stats.length < 200 {
        "WOW! That's a third leg, not a dick! Special pants required?"
    } else {
        "LEGENDARY! Scientists want to study this mutation. BEWARE!"
    };

    let escaped_target_name = escape_markdown(&target_user.name);
    let description = if is_self {
        "Here's everything you wanted to know about your cucumber (and probably some things you didn't):".to_string()
    } else {
        format!(
            "Here's everything to know about {}'s cucumber:",
            escaped_target_name
        )
    };

    let footer_text = if is_self {
        "Remember to /grow every day for maximum results!"
    } else {
        "Use /stats without parameters to see your own stats!"
    };

    let builder = CreateInteractionResponse::Message(
        CreateInteractionResponseMessage::new()
            .add_embed(
                CreateEmbed::new()
                    .title(format!("🍆 {}'s Dick Stats", escaped_target_name))
                    .description(description)
                    .color(0x9B59B6) // Purple
                    .field("Current Length", format!("**{} cm**", user_stats.length), true)
                    .field("Server Rank", format!("**{}{}**", rank, ordinal_suffix(rank)), true)
                    .field("Title", fun_title, true)
                    .field(
                        "Dick of the Day",
                        format!("**{} time(s)**", user_stats.dick_of_day_count),
                        true,
                    )
                    .field("Growth Status", growth_status, false)
                    .field("Viagra Status", viagra_status, true)
                    .field(
                        "Battle Stats",
                        format!(
                            "Win rate: **{:.2}%**\nFights: **{}**\nWins: **{}**\nMax win streak: **{}**\nCurrent streak: **{}**\nAcquired length: **{} cm**\nLost length: **{} cm**",
                            win_rate,
                            total_fights,
                            user_stats.pvp_wins,
                            user_stats.pvp_max_streak,
                            user_stats.pvp_current_streak,
                            user_stats.cm_won,
                            user_stats.cm_lost
                        ),
                        false
                    )
                    .field("Professional Assessment", length_comment, false)
                    .thumbnail(target_user.face())
                    .footer(CreateEmbedFooter::new(footer_text)),
            )
    );
    command.create_response(&ctx.http, builder).await
}
