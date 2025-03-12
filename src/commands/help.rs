use serenity::all::{
    CommandInteraction, CreateEmbed, CreateEmbedFooter, CreateInteractionResponse,
    CreateInteractionResponseMessage,
};
use serenity::prelude::*;

pub async fn handle_help_command(
    _ctx: &Context,
    _command: &CommandInteraction,
) -> CreateInteractionResponse {
    CreateInteractionResponse::Message(
        CreateInteractionResponseMessage::new()
            .add_embed(
                CreateEmbed::new()
                    .title("🍆 Cucumber Bot Help Guide 🍆")
                    .description(
                        "Welcome to the Cucumber Bot - where size matters and every day is a new opportunity to grow! Below you'll find information about all the available commands:"
                    )
                    .color(0x9B59B6) // Purple
                    .field(
                        "/grow", 
                        "Grow your cucumber once per day. Your length can increase or decrease randomly (-5 to +10 cm). The command resets at 00:00 UTC every day.", 
                        false
                    )
                    .field(
                        "/top", 
                        "Shows the leaderboard of the biggest dicks in the current server.", 
                        false
                    )
                    .field(
                        "/global", 
                        "Shows the leaderboard of the biggest dicks across all servers where the bot is used.", 
                        false
                    )
                    .field(
                        "/pvp", 
                        "Start a dick measuring contest with someone. Enter the amount of centimeters you want to bet. If you win, you gain that length from your opponent. If you lose, you lose that length to them.", 
                        false
                    )
                    .field(
                        "/stats", 
                        "View your dick stats including length, rank, win/loss record, and more.", 
                        false
                    )
                    .field(
                        "/dickoftheday", 
                        "Randomly selects one active user to be the Dick of the Day, granting them a bonus of 5-10 cm. This command can only be used once per server per day (resets at 00:00 UTC).", 
                        false
                    )
                    .field(
                        "Daily Reset", 
                        "Both the `/grow` command and `/dickoftheday` command reset at midnight UTC (00:00 UTC) every day.", 
                        false
                    )
                    .footer(CreateEmbedFooter::new("May your cucumber grow long and prosperous! 🥒")),
            )
            .ephemeral(true),
    )
}
