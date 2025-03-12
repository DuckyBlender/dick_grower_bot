use crate::Bot;
use crate::time::check_30_minutes;
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
) -> CreateInteractionResponse {
    let data = ctx.data.read().await;
    let bot = data.get::<Bot>().unwrap();

    let user_id = command.user.id.to_string();
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
            return CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .add_embed(
                        CreateEmbed::new()
                            .title("❓ No Stats Found")
                            .description(
                                "You haven't started growing your dick yet! Use /grow to begin your journey to greatness.",
                            )
                            .color(0xAAAAAA),
                    )
                    .ephemeral(true),
            );
        }
        Err(why) => {
            error!("Database error: {:?}", why);
            return CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .add_embed(
                        CreateEmbed::new()
                            .title("⚠️ Database Error")
                            .description("Failed to retrieve your stats. The server's ruler broke.")
                            .color(0xFF0000),
                    )
                    .ephemeral(true),
            );
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

    // Calculate growth status
    let last_grow = NaiveDateTime::parse_from_str(&user_stats.last_grow, "%Y-%m-%d %H:%M:%S")
        .unwrap_or_default();

    // Check if user can grow today
    let time_left = check_30_minutes(&last_grow);
    let growth_status = if time_left.0 {
        "✅ You can grow now! Use /grow".to_string()
    } else {
        format!(
            "⏰ Next growth in **{}h {}m**",
            time_left.1.num_hours(),
            time_left.1.num_minutes() % 60
        )
    };

    // Calculate win rate
    let total_fights = user_stats.pvp_wins + user_stats.pvp_losses;
    let win_rate = if total_fights > 0 {
        (user_stats.pvp_wins as f64 / total_fights as f64) * 100.0
    } else {
        0.0
    };

    // Funny comment based on length
    let length_comment = if user_stats.length <= 0 {
        "Your dick is practically an innie at this point. Keep trying!"
    } else if user_stats.length < 10 {
        "It's... cute? At least that's what they'll say to be nice."
    } else if user_stats.length < 20 {
        "Not bad! You're in the average zone. But who wants to be average?"
    } else if user_stats.length < 30 {
        "Impressive length! You're packing some serious heat down there."
    } else if user_stats.length < 50 {
        "WOW! That's a third leg, not a dick! Do you need special pants?"
    } else {
        "LEGENDARY! Scientists want to study your mutation. BEWARE!"
    };

    CreateInteractionResponse::Message(
        CreateInteractionResponseMessage::new()
            .add_embed(
                CreateEmbed::new()
                    .title(format!("🍆 {}'s Dick Stats", command.user.name))
                    .description(
                        "Here's everything you wanted to know about your cucumber (and probably some things you didn't):".to_string(),
                    )
                    .color(0x9B59B6) // Purple
                    .field("Current Length", format!("**{} cm**", user_stats.length), true)
                    .field("Server Rank", format!("**#{}**", rank), true)
                    .field(
                        "Dick of the Day",
                        format!("**{} time(s)**", user_stats.dick_of_day_count),
                        true,
                    )
                    .field("Growth Status", growth_status, false)
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
                    .thumbnail(command.user.face())
                    .footer(CreateEmbedFooter::new("Remember to /grow every day for maximum results!")),
            )
            .ephemeral(true),
    )
}
