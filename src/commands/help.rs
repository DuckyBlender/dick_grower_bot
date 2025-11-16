use serenity::all::{
    CommandInteraction, CreateEmbed, CreateEmbedFooter, CreateInteractionResponse,
    CreateInteractionResponseMessage,
};
use serenity::prelude::*;

pub async fn handle_help_command(
    ctx: &Context,
    command: &CommandInteraction,
) -> Result<(), serenity::Error> {
    let description = "\
        `/grow` - Grow your plant once per 60 minutes (always positive growth now!)\n\
        `/top` - View the server's plant leaderboard\n\
        `/global` - View the global plant leaderboard\n\
        `/pvp <bet>` - Challenge someone to a plant battle with a cm bet\n\
        `/stats <user>` - View your or someone else's plant stats\n\
        `/plantoftheday` - Select a random Plant of the Day\n\
        `/gift <user> <amount>` - Gift some of your growth to another user\n\
        `/prestige` - Prestige your plant to gain bonuses\n\
        `/settings` - Configure bot settings for this server\n\
        `/viagra` - Boost your growth by 20% for 6 hours (20 hour cooldown)\n\
        `/help` - Show this help message\n\
        \n\
        **🔔 Bot Updates & Community:**\n\
        Join our Discord for announcements and other projects: [Discord Server](https://discord.gg/39nqUzYGbe)\n\
    ";

    let builder = CreateInteractionResponse::Message(
        CreateInteractionResponseMessage::new().add_embed(
            CreateEmbed::new()
                .title("🌱 Plant Grower Bot Help")
                .description(description)
                .color(0x00FF00)
                .footer(CreateEmbedFooter::new(
                    "Compete with friends for the biggest plant in town!",
                )),
        ),
    );

    return command.create_response(&ctx.http, builder).await;
}
