use anyhow::Context as _;
use futures::future::try_join_all;
use serenity::{
    all::CommandInteraction, builder::{
        CreateCommand, CreateEmbed, CreateEmbedFooter, CreateInteractionResponse,
        CreateInteractionResponseFollowup, CreateInteractionResponseMessage,
    }, client::Context, model::Permissions
};

use crate::services::{
    adm_service::{AdmService, Status},
    information_service::InformationService,
};

pub const COMMAND_NAME: &'static str = "adm";

pub async fn run(
    ctx: &Context,
    interaction: &CommandInteraction,
    information: &InformationService,
    adm_service: &AdmService,
) -> anyhow::Result<()> {
    interaction
        .create_response(
            &ctx.http,
            CreateInteractionResponse::Defer(CreateInteractionResponseMessage::new()),
        )
        .await
        .expect("create response");

    let system_adms = adm_service.get_adm_status().await;

    let critical_systems: Vec<_> = system_adms
        .iter()
        .filter_map(|system_adm| match system_adm.status {
            Status::Critical(_) => Some(system_adm),
            _ => None,
        })
        .collect();

    let critical_system_names = try_join_all(
        critical_systems
            .iter()
            .map(|system| information.get_system(system.system_id)),
    )
    .await
    .context("get system names")?
    .iter()
    .map(|system| system.name.to_owned())
    .reduce(|acc, system_name| format!("{}, {}", acc, system_name));

    let warning_systems: Vec<_> = system_adms
        .iter()
        .filter_map(|system_adm| match system_adm.status {
            Status::Warning(_) => Some(system_adm),
            _ => None,
        })
        .collect();

    let warning_system_names = try_join_all(
        warning_systems
            .iter()
            .map(|system| information.get_system(system.system_id)),
    )
    .await
    .context("get system names")?
    .iter()
    .map(|system| system.name.to_owned())
    .reduce(|acc, system_name| format!("{}, {}", acc, system_name));

    let embed = CreateEmbed::new()
        .title("ADM Status Report")
        .field("Critical Systems", critical_system_names.unwrap_or("None 🏆".to_string()), false)
        .field("Warning Systems", warning_system_names.unwrap_or("None 🎉".to_string()), false)
        .footer(CreateEmbedFooter::new("🦀 Please focus on the <Critical> systems first and then move on to the <Warning> systems."));

    interaction
        .create_followup(
            &ctx.http,
            CreateInteractionResponseFollowup::new().embed(embed),
        )
        .await?;

    Ok(())
}

pub fn register() -> CreateCommand {
    CreateCommand::new(COMMAND_NAME)
        .description("Show which systems require attention due to low ADM.")
        .default_member_permissions(Permissions::SEND_MESSAGES)
        .dm_permission(true)
}
