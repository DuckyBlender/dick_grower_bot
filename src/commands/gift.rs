use crate::Bot;
use crate::commands::escape_markdown;
use log::{error, info};
use serenity::all::{
    CommandInteraction, CreateEmbed, CreateEmbedFooter, CreateInteractionResponse,
    CreateInteractionResponseMessage, Mentionable,
};
use serenity::prelude::*;

pub async fn handle_gift_command(
    ctx: &Context,
    command: &CommandInteraction,
) -> Result<(), serenity::Error> {
    let data = ctx.data.read().await;
    let bot = data.get::<Bot>().unwrap();

    let options = &command.data.options;
    let recipient_user = options[0].value.as_user_id().unwrap();
    let amount = options[1].value.as_i64().unwrap();

    let giver_id = command.user.id;
    let guild_id = command.guild_id.unwrap();

    // Validate amount
    if amount <= 0 {
        let builder = CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .add_embed(
                    CreateEmbed::new()
                        .title("❌ Invalid Gift Amount")
                        .description("You need to gift at least 1 cm! Don't be so stingy with your length.")
                        .color(0xFF0000),
                )
                .ephemeral(true),
        );
        return command.create_response(&ctx.http, builder).await;
    }

    // Check for self-gifting
    if giver_id == recipient_user {
        let builder = CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .add_embed(
                    CreateEmbed::new()
                        .title("🤨 Self-Gift Detected")
                        .description("You can't gift yourself! That would defeat the purpose of generosity.")
                        .color(0xFF9900),
                )
                .ephemeral(true),
        );
        return command.create_response(&ctx.http, builder).await;
    }

    let giver_id_str = giver_id.to_string();
    let recipient_id_str = recipient_user.to_string();
    let guild_id_str = guild_id.to_string();

    // Check if giver has enough length
    let giver_length = match sqlx::query!(
        "SELECT length FROM dicks WHERE user_id = ? AND guild_id = ?",
        giver_id_str,
        guild_id_str
    )
    .fetch_optional(&bot.database)
    .await
    {
        Ok(Some(record)) => record.length,
        Ok(None) => {
            // Create new user with 0 length
            info!(
                "New giver detected, adding user {} ({}) in guild id {} to database",
                command.user.name, giver_id, guild_id
            );
            match sqlx::query!(
                "INSERT INTO dicks (user_id, guild_id, length, last_grow, growth_count, dick_of_day_count, 
                                   pvp_wins, pvp_losses, pvp_max_streak, pvp_current_streak,
                                   cm_won, cm_lost)
                 VALUES (?, ?, 0, datetime('now', '-2 days'), 0, 0, 0, 0, 0, 0, 0, 0)",
                giver_id_str,
                guild_id_str
            )
            .execute(&bot.database)
            .await
            {
                Ok(_) => 0,
                Err(why) => {
                    error!("Error creating giver user: {:?}", why);
                    0
                }
            }
        }
        Err(why) => {
            error!("Database error checking giver: {:?}", why);
            let builder = CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .add_embed(
                        CreateEmbed::new()
                            .title("⚠️ Database Error")
                            .description("Failed to check your length. The measuring tape broke.")
                            .color(0xFF0000),
                    )
                    .ephemeral(true),
            );
            return command.create_response(&ctx.http, builder).await;
        }
    };

    if giver_length < amount {
        let builder = CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .add_embed(
                    CreateEmbed::new()
                        .title("❌ Insufficient Length")
                        .description(format!(
                            "You only have **{} cm** but you're trying to gift **{} cm**!\n\nYou can't give what you don't have. Grow more first!",
                            giver_length, amount
                        ))
                        .color(0xFF0000),
                )
                .ephemeral(true),
        );
        return command.create_response(&ctx.http, builder).await;
    }

    // Check if recipient exists, if not create them
    let _recipient_exists = match sqlx::query!(
        "SELECT length FROM dicks WHERE user_id = ? AND guild_id = ?",
        recipient_id_str,
        guild_id_str
    )
    .fetch_optional(&bot.database)
    .await
    {
        Ok(Some(_)) => true,
        Ok(None) => {
            // Create new recipient user
            let recipient = match recipient_user.to_user(ctx).await {
                Ok(user) => user,
                Err(_) => {
                    let builder = CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new()
                            .add_embed(
                                CreateEmbed::new()
                                    .title("❌ User Not Found")
                                    .description("Could not find the recipient user.")
                                    .color(0xFF0000),
                            )
                            .ephemeral(true),
                    );
                    return command.create_response(&ctx.http, builder).await;
                }
            };

            info!(
                "New recipient detected, adding user {} ({}) in guild id {} to database",
                recipient.name, recipient_user, guild_id
            );
            match sqlx::query!(
                "INSERT INTO dicks (user_id, guild_id, length, last_grow, growth_count, dick_of_day_count, 
                                   pvp_wins, pvp_losses, pvp_max_streak, pvp_current_streak,
                                   cm_won, cm_lost)
                 VALUES (?, ?, 0, datetime('now', '-2 days'), 0, 0, 0, 0, 0, 0, 0, 0)",
                recipient_id_str,
                guild_id_str
            )
            .execute(&bot.database)
            .await
            {
                Ok(_) => false,
                Err(why) => {
                    error!("Error creating recipient user: {:?}", why);
                    false
                }
            }
        }
        Err(why) => {
            error!("Database error checking recipient: {:?}", why);
            let builder = CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .add_embed(
                        CreateEmbed::new()
                            .title("⚠️ Database Error")
                            .description("Failed to check recipient.")
                            .color(0xFF0000),
                    )
                    .ephemeral(true),
            );
            return command.create_response(&ctx.http, builder).await;
        }
    };

    // Perform the transfer
    let mut transaction = match bot.database.begin().await {
        Ok(tx) => tx,
        Err(why) => {
            error!("Error starting transaction: {:?}", why);
            let builder = CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .add_embed(
                        CreateEmbed::new()
                            .title("⚠️ Database Error")
                            .description("Failed to start gift transaction.")
                            .color(0xFF0000),
                    )
                    .ephemeral(true),
            );
            return command.create_response(&ctx.http, builder).await;
        }
    };

    // Remove from giver
    if let Err(why) = sqlx::query!(
        "UPDATE dicks SET length = length - ? WHERE user_id = ? AND guild_id = ?",
        amount,
        giver_id_str,
        guild_id_str
    )
    .execute(&mut *transaction)
    .await
    {
        error!("Error removing length from giver: {:?}", why);
        let _ = transaction.rollback().await;
        let builder = CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .add_embed(
                    CreateEmbed::new()
                        .title("⚠️ Gift Failed")
                        .description("Failed to process the gift. Transaction rolled back.")
                        .color(0xFF0000),
                )
                .ephemeral(true),
        );
        return command.create_response(&ctx.http, builder).await;
    }

    // Add to recipient
    if let Err(why) = sqlx::query!(
        "UPDATE dicks SET length = length + ? WHERE user_id = ? AND guild_id = ?",
        amount,
        recipient_id_str,
        guild_id_str
    )
    .execute(&mut *transaction)
    .await
    {
        error!("Error adding length to recipient: {:?}", why);
        let _ = transaction.rollback().await;
        let builder = CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .add_embed(
                    CreateEmbed::new()
                        .title("⚠️ Gift Failed")
                        .description("Failed to process the gift. Transaction rolled back.")
                        .color(0xFF0000),
                )
                .ephemeral(true),
        );
        return command.create_response(&ctx.http, builder).await;
    }

    // Get new lengths for history logging
    let giver_new_length = match sqlx::query!(
        "SELECT length FROM dicks WHERE user_id = ? AND guild_id = ?",
        giver_id_str,
        guild_id_str
    )
    .fetch_one(&mut *transaction)
    .await
    {
        Ok(record) => record.length,
        Err(_) => giver_length - amount, // fallback
    };

    let recipient_new_length = match sqlx::query!(
        "SELECT length FROM dicks WHERE user_id = ? AND guild_id = ?",
        recipient_id_str,
        guild_id_str
    )
    .fetch_one(&mut *transaction)
    .await
    {
        Ok(record) => record.length,
        Err(_) => amount, // fallback for new users
    };

    // Log the gift in length_history for both users
    let negative_amount = -amount;
    if let Err(why) = sqlx::query!(
        "INSERT INTO length_history (user_id, guild_id, length, growth_amount, growth_type)
         VALUES (?, ?, ?, ?, 'gift_sent')",
        giver_id_str,
        guild_id_str,
        giver_new_length,
        negative_amount
    )
    .execute(&mut *transaction)
    .await
    {
        error!("Error logging gift sent history: {:?}", why);
    }

    if let Err(why) = sqlx::query!(
        "INSERT INTO length_history (user_id, guild_id, length, growth_amount, growth_type)
         VALUES (?, ?, ?, ?, 'gift_received')",
        recipient_id_str,
        guild_id_str,
        recipient_new_length,
        amount
    )
    .execute(&mut *transaction)
    .await
    {
        error!("Error logging gift received history: {:?}", why);
    }

    // Commit transaction
    if let Err(why) = transaction.commit().await {
        error!("Error committing gift transaction: {:?}", why);
        let builder = CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .add_embed(
                    CreateEmbed::new()
                        .title("⚠️ Gift Failed")
                        .description("Failed to commit the gift transaction.")
                        .color(0xFF0000),
                )
                .ephemeral(true),
        );
        return command.create_response(&ctx.http, builder).await;
    }

    // Get usernames for display
    let giver_mention = giver_id.mention();
    let recipient_mention = recipient_user.mention();
    let _recipient = match recipient_user.to_user(ctx).await {
        Ok(user) => escape_markdown(&user.name),
        Err(_) => "Unknown User".to_string(),
    };

    // Create funny comments based on gift size
    let gift_comment = if amount >= 50 {
        "What an incredibly generous donation! This kind of philanthropy will go down in history!"
    } else if amount >= 25 {
        "That's a substantial gift! Your generosity knows no bounds!"
    } else if amount >= 10 {
        "A respectable gift! The recipient will surely appreciate your kindness."
    } else if amount >= 5 {
        "A modest but thoughtful gift. Every centimeter counts!"
    } else {
        "A small token of appreciation. It's the thought that counts... right?"
    };

    let builder = CreateInteractionResponse::Message(
        CreateInteractionResponseMessage::new()
            .content(format!("{} {}", giver_mention, recipient_mention))
            .add_embed(
                CreateEmbed::new()
                    .title("🎁 Gift Successfully Sent!")
                    .description(format!(
                        "{} has generously gifted **{} cm** to {}!\n\n{}\n\n**New Lengths:**\n• Giver: {} cm\n• Recipient: {} cm",
                        giver_mention, amount, recipient_mention, gift_comment, giver_new_length, recipient_new_length
                    ))
                    .color(0x00FF00) // Green
                    .footer(CreateEmbedFooter::new("Sharing is caring! Spread the love (and the length)!")),
            ),
    );
    return command.create_response(&ctx.http, builder).await;
} 