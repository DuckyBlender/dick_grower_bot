use serenity::all::{
    CommandInteraction, CreateEmbed, CreateEmbedFooter, CreateInteractionResponse,
    CreateInteractionResponseMessage,
};
use serenity::prelude::*;

pub async fn handle_help_command(
    _ctx: &Context,
    _command: &CommandInteraction,
) -> CreateInteractionResponse {
    let description = "\
        **🍆 Dick Grower Bot Commands:**\n\
        \n\
        `/grow` - Grow your dick once per day\n\
        `/top` - View the server's dick leaderboard\n\
        `/global` - View the global dick leaderboard\n\
        `/pvp <bet>` - Challenge someone to a dick battle with a cm bet\n\
        `/gift <user> <amount>` - Gift some cm to another user\n\
        `/stats [user]` - View your or someone else's dick stats\n\
        `/dickoftheday` - Select a random Dick of the Day\n\
        `/help` - Show this help message\n\
        \n\
        **🔔 Bot Updates & Community:**\n\
        Join our Discord for announcements and other projects: [Discord Server](https://discord.gg/39nqUzYGbe)\n\
    ";

    CreateInteractionResponse::Message(
        CreateInteractionResponseMessage::new().add_embed(
            CreateEmbed::new()
                .title("🍆 Dick Grower Bot Help")
                .description(description)
                .color(0x00FF00)
                .footer(CreateEmbedFooter::new("Compete with friends for the biggest dick in town!")),
        ),
    )
}
