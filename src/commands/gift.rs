use crate::Bot;
use log::{error, info};
use serenity::all::{
    CommandInteraction, CreateEmbed, CreateEmbedFooter, CreateInteractionResponse,
    CreateInteractionResponseMessage,
};
use serenity::model::id::UserId;
use serenity::prelude::*;

pub async fn handle_gift_command(
    ctx: &Context,
    command: &CommandInteraction,
) -> Result<(), serenity::Error>  {
    let data = ctx.data.read().await;
    let bot = data.get::<Bot>().unwrap();

    let options = &command.data.options;
    info!("Options: {:?}", options);

    // Safely extract options
    let user_option = match options.iter().find(|o| o.name == "user") {
        Some(option) => option,
        None => {
            let builder = CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .add_embed(
                        CreateEmbed::new()
                            .title("❌ Missing User")
                            .description("You need to specify a user to gift centimeters to!")
                            .color(0xFF0000),
                    )
                    .ephemeral(true),
            );
            return command.create_response(&ctx.http, builder).await;
        }
    };

    let amount_option = match options.iter().find(|o| o.name == "amount") {
        Some(option) => option,
        None => {
            let builder = CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .add_embed(
                        CreateEmbed::new()
                            .title("❌ Missing Amount")
                            .description("You need to specify how many centimeters to gift!")
                            .color(0xFF0000),
                    )
                    .ephemeral(true),
            );
            return command.create_response(&ctx.http, builder).await;
        }
    };

    // Extract the user ID from the user option
    let user_id = match user_option.value.as_user_id() {
        Some(id) => id.to_string(),
        None => {
            let builder = CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .add_embed(
                        CreateEmbed::new()
                            .title("❌ Invalid User")
                            .description("Could not parse the user ID.")
                            .color(0xFF0000),
                    )
                    .ephemeral(true),
            );
            return command.create_response(&ctx.http, builder).await;
        }
    };

    let amount = match amount_option.value.as_i64() {
        Some(val) => val,
        None => {
            let builder = CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .add_embed(
                        CreateEmbed::new()
                            .title("❌ Invalid Amount")
                            .description("The amount must be a valid number.")
                            .color(0xFF0000),
                    )
                    .ephemeral(true),
            );
            return command.create_response(&ctx.http, builder).await;
        }
    };

    // Check if the amount is valid, should be verified by discord but never trust front end input
    if amount < 1 {
        let builder = CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .add_embed(
                    CreateEmbed::new()
                        .title("❌ Invalid Amount")
                        .description("You need to gift at least 1 cm!")
                        .color(0xFF0000),
                )
                .ephemeral(true),
        );
        return command.create_response(&ctx.http, builder).await;
    }

    let guild_id = command.guild_id.unwrap().to_string();
    let sender_id = command.user.id.to_string();

    // Check if the sender has enough length
    let sender_length = match sqlx::query!(
        "SELECT length FROM dicks WHERE user_id = ? AND guild_id = ?",
        sender_id,
        guild_id
    )
    .fetch_optional(&bot.database)
    .await
    {
        Ok(Some(record)) => record.length,
        Ok(None) => {
            // Create new user
            info!(
                "New user detected, adding user {} ({}) in guild id {} to database",
                command.user.name, sender_id, guild_id
            );
            match sqlx::query!(
                "INSERT INTO dicks (user_id, guild_id, length, last_grow, dick_of_day_count, 
                                   pvp_wins, pvp_losses, pvp_max_streak, pvp_current_streak,
                                   cm_won, cm_lost)
                 VALUES (?, ?, 0, datetime('now', '-2 days'), 0, 0, 0, 0, 0, 0, 0)",
                sender_id,
                guild_id
            )
            .execute(&bot.database)
            .await
            {
                Ok(_) => 0,
                Err(why) => {
                    error!("Error creating user: {:?}", why);
                    0
                }
            }
        }
        Err(why) => {
            error!("Database error: {:?}", why);
            let builder = CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .add_embed(
                        CreateEmbed::new()
                            .title("⚠️ Database Error")
                            .description("Failed to check your length.")
                            .color(0xFF0000),
                    )
                    .ephemeral(true),
            );
            return command.create_response(&ctx.http, builder).await;
        }
    };

    if sender_length < amount {
        let builder = CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .add_embed(
                    CreateEmbed::new()
                        .title("❌ Insufficient Length")
                        .description(format!(
                            "You only have **{} cm** but you're trying to gift **{} cm**!\n\nYou can't gift what you don't have, buddy. Your ambition outweighs your equipment.",
                            sender_length, amount
                        ))
                        .color(0xFF0000),
                )
                .ephemeral(true),
        );
        return command.create_response(&ctx.http, builder).await;
    }

    // Update the database for the sender
    match sqlx::query!(
        "UPDATE dicks SET length = length - ? WHERE user_id = ? AND guild_id = ?",
        amount,
        sender_id,
        guild_id
    )
    .execute(&bot.database)
    .await
    {
        Ok(_) => (),
        Err(why) => {
            error!("Error updating sender: {:?}", why);
            let builder = CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .add_embed(
                        CreateEmbed::new()
                            .title("⚠️ Database Error")
                            .description("Failed to update your length.")
                            .color(0xFF0000),
                    )
                    .ephemeral(true),
            );
            return command.create_response(&ctx.http, builder).await;
        }
    };

    // Update the database for the receiver
    match sqlx::query!(
        "UPDATE dicks SET length = length + ? WHERE user_id = ? AND guild_id = ?",
        amount,
        user_id,
        guild_id
    )
    .execute(&bot.database)
    .await
    {
        Ok(_) => (),
        Err(why) => {
            error!("Error updating receiver: {:?}", why);
            let builder = CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .add_embed(
                        CreateEmbed::new()
                            .title("⚠️ Database Error")
                            .description("Failed to update the receiver's length.")
                            .color(0xFF0000),
                    )
                    .ephemeral(true),
            );
            return command.create_response(&ctx.http, builder).await;
        }
    };

    // Get receiver's new length
    let receiver_length = match sqlx::query!(
        "SELECT length FROM dicks WHERE user_id = ? AND guild_id = ?",
        user_id,
        guild_id
    )
    .fetch_one(&bot.database)
    .await
    {
        Ok(record) => record.length,
        Err(_) => 0,
    };

    // Get sender's new length
    let sender_length = match sqlx::query!(
        "SELECT length FROM dicks WHERE user_id = ? AND guild_id = ?",
        sender_id,
        guild_id
    )
    .fetch_one(&bot.database)
    .await
    {
        Ok(record) => record.length,
        Err(_) => 0,
    };

    // Get receiver's username
    let receiver = match UserId::new(user_id.parse::<u64>().unwrap_or_default())
        .to_user(&ctx)
        .await
    {
        Ok(user) => user.name,
        Err(_) => "Unknown User".to_string(),
    };

    let builder = CreateInteractionResponse::Message(
        CreateInteractionResponseMessage::new()
            .add_embed(
                CreateEmbed::new()
                    .title("🎁 Gifted Centimeters!")
                    .description(format!(
                        "You gifted **{} cm** to **{}**!\n\n**Your new length**: **{} cm**\n**{}'s new length**: **{} cm**",
                        amount, 
                        receiver, 
                        sender_length, 
                        receiver, 
                        receiver_length + amount
                    ))
                    .color(0x3498DB) // Blue
                    .footer(CreateEmbedFooter::new("Generosity is the best policy!")),
            )
            .ephemeral(true),
    );
    return command.create_response(&ctx.http, builder).await;
}
