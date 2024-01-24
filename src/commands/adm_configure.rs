use std::time::Duration;

use serenity::{
    all::CommandInteraction,
    builder::{
        CreateCommand, CreateEmbed, CreateInteractionResponse, CreateInteractionResponseMessage,
    },
    client::Context,
    model::Permissions,
    utils::CreateQuickModal,
};

use crate::services::adm_configuration::{AdmConfiguration, Importance};

pub const COMMAND_NAME: &'static str = "adm_configure";

pub async fn run(
    ctx: &Context,
    interaction: &CommandInteraction,
    adm_configuration: &AdmConfiguration,
) -> anyhow::Result<()> {
    let modal = CreateQuickModal::new("Configure ADM")
        .timeout(Duration::from_secs(600))
        .short_field("System")
        .short_field("Importance (Red, Yellow, Green)");

    let response = interaction.quick_modal(ctx, modal).await?;

    if let Some(response) = response {
        let system = response.inputs[0].to_uppercase();
        let importance = match response.inputs[1].to_uppercase().as_str() {
            "RED" => Some(Importance::Red),
            "YELLOW" => Some(Importance::Yellow),
            "GREEN" => Some(Importance::Green),
            _ => None,
        };

        if let Some(importance) = importance {
            adm_configuration
                .set_importance(&system, importance)
                .await?;

            response
                .interaction
                .create_response(
                    &ctx.http,
                    CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new()
                            .embed(
                                CreateEmbed::new()
                                    .title("System Importance Updated")
                                    .field("System", system, true)
                                    .field("Importance", format!("{}", importance), true),
                            )
                            .ephemeral(true),
                    ),
                )
                .await?;
        } else {
            response.interaction.create_response(
                &ctx.http,
                CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .content(
                            "Unrecognized importance level, please use `Red`, `Yellow`, or `Green`",
                        )
                        .ephemeral(true),
                ),
            ).await?;
        }
    } else {
        tracing::warn!("modal response is `None`");
    }

    Ok(())
}

pub fn register() -> CreateCommand {
    CreateCommand::new(COMMAND_NAME)
        .description("Configure ADM importance of systems.")
        .default_member_permissions(Permissions::ADMINISTRATOR)
}
