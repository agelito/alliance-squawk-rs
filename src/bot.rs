use std::env;
use std::sync::Arc;

use serenity::all::{ChannelId, GuildId, Interaction};
use serenity::async_trait;
use serenity::builder::{
    CreateEmbed, CreateEmbedFooter, CreateInteractionResponse, CreateInteractionResponseMessage,
    CreateMessage,
};
use serenity::model::gateway::Ready;
use serenity::prelude::*;
use tokio::sync::mpsc::UnboundedReceiver;

use crate::commands;
use crate::esi::EsiID;
use crate::services::adm_configuration::AdmConfiguration;
use crate::services::adm_service::{AdmService, Status, SystemAdm};
use crate::services::information_service::InformationService;

#[allow(dead_code)]
pub enum BotNotification {
    NotifyCorpJoinAlliance(EsiID, EsiID),
    NotifyCorpLeftAlliance(EsiID, EsiID),
    NotifyAdm(SystemAdm),
}

pub type BotResult = anyhow::Result<()>;

struct Bot {
    channel_id: u64,
    information: InformationService,
    adm_service: AdmService,
    adm_configuration: AdmConfiguration,
    command_receiver: RwLock<Option<UnboundedReceiver<BotNotification>>>,
}

#[async_trait]
impl EventHandler for Bot {
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::Command(command) = interaction {
            tracing::info!(
                name = command.data.name,
                user = command.user.name,
                "command interaction"
            );

            let content = match command.data.name.as_str() {
                commands::adm::COMMAND_NAME => {
                    commands::adm::run(&ctx, &command, &self.information, &self.adm_service)
                        .await
                        .unwrap();

                    None
                }
                commands::adm_configure::COMMAND_NAME => {
                    commands::adm_configure::run(&ctx, &command, &self.adm_configuration).await.unwrap();

                    None
                }
                _ => Some("Command not implemented!".to_string()),
            };

            if let Some(content) = content {
                let data = CreateInteractionResponseMessage::new().content(content);
                let builder = CreateInteractionResponse::Message(data);
                if let Err(why) = command.create_response(&ctx.http, builder).await {
                    tracing::error!(?why, "couldn't create command response");
                }
            }
        }
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        tracing::info!(bot_name = ready.user.name, "connected");

        let guild_id = GuildId::new(
            env::var("DISCORD_GUILD_ID")
                .expect("`DISCORD_GUILD_ID` configuration variable")
                .parse()
                .expect("`DISCORD_GUILD_ID` is an integer"),
        );

        let commands = guild_id
            .set_commands(
                &ctx.http,
                vec![
                    commands::adm::register(),
                    commands::adm_configure::register(),
                ],
            )
            .await;

        tracing::info!(?guild_id, ?commands, "registered commands");

        if let Some(mut receiver) = self.command_receiver.write().await.take() {
            let information = self.information.clone();

            let ctx = Arc::new(ctx);
            let channel_id = self.channel_id;

            tokio::spawn(async move {
                loop {
                    let command = receiver.recv().await;

                    match command {
                        Some(command) => {
                            send_notification(&ctx, channel_id, &information, command).await
                        }
                        None => {
                            tracing::warn!("channel closed, stopping command loop");
                            break;
                        }
                    };
                }
            });
        }
    }
}

async fn send_corp_notification(
    ctx: &Context,
    channel_id: u64,
    info: &InformationService,
    alliance_id: EsiID,
    corporation_id: EsiID,
    msg: &str,
) {
    tracing::info!(alliance_id, corporation_id, msg, "send corp notification");

    let res = tokio::try_join!(
        info.get_alliance(alliance_id),
        info.get_corporation(corporation_id)
    );

    match res {
        Ok((alliance, corporation)) => {
            tracing::debug!(alliance_id, corporation_id, "esi data");

            if corporation.member_count < 10 {
                return;
            }

            let alliance_link = format!(
                "https://evemaps.dotlan.net/alliance/{}",
                alliance.name.replace(' ', "_")
            );
            let corporation_link = format!(
                "https://evemaps.dotlan.net/corp/{}",
                corporation.name.replace(' ', "_")
            );

            let embed = CreateEmbed::new()
                .title(msg)
                .field(
                    "Corporation",
                    format!(
                        "{} ([{}]({}))",
                        corporation.name, corporation.ticker, corporation_link
                    ),
                    false,
                )
                .field(
                    "Member Count",
                    format!("{}", corporation.member_count),
                    false,
                )
                .field(
                    "Alliance",
                    format!(
                        "{} ([{}]({}))",
                        alliance.name, alliance.ticker, alliance_link
                    ),
                    false,
                )
                .color((188, 69, 255));

            let builder = CreateMessage::new().embed(embed);
            let message = ChannelId::new(channel_id).send_message(&ctx, builder).await;

            tracing::debug!(?message, "composed message");

            if let Err(err) = message {
                tracing::error!(?err, "error sending notification");
            }
        }
        Err(err) => tracing::error!(?err, "error fetching esi data"),
    }
}

async fn send_adm_notification(
    ctx: &Context,
    channel_id: u64,
    info: &InformationService,
    system_adm: SystemAdm,
) {
    tracing::info!(?system_adm, "send adm notification");

    match info.get_system(system_adm.system_id).await {
        Ok(system) => {
            if let Some((msg, footer, adm, color)) = match system_adm.status {
                Status::Warning(adm) => Some((
                    format!("{} ADM is deteriorated!", system.name),
                    "Please do some ratting or mining here.",
                    adm,
                    (238, 210, 2),
                )),
                Status::Critical(adm) => Some((
                    format!("{} ADM is critically low!", system.name),
                    "Do ratting or mining here ASAP!!!",
                    adm,
                    (255, 103, 0),
                )),
                _ => None,
            } {
                let system_link = format!("https://evemaps.dotlan.net/system/{}", system.name);

                let embed = CreateEmbed::new()
                    .title(msg)
                    .field(
                        "System",
                        format!("[{}]({})", system.name, system_link),
                        true,
                    )
                    .field("ADM", format!("{}", adm), true)
                    .footer(CreateEmbedFooter::new(footer))
                    .color(color);

                let builder = CreateMessage::new().embed(embed);
                let message = ChannelId::new(channel_id).send_message(&ctx, builder).await;

                tracing::debug!(?message, "composed message");

                if let Err(err) = message {
                    tracing::error!(?err, "error sending notification");
                }
            }
        }
        Err(err) => tracing::error!(?err, "error fetching esi data"),
    }
}

async fn send_notification(
    ctx: &Context,
    channel_id: u64,
    info: &InformationService,
    command: BotNotification,
) {
    match command {
        BotNotification::NotifyCorpJoinAlliance(alliance_id, corporation_id) => {
            send_corp_notification(
                ctx,
                channel_id,
                info,
                alliance_id,
                corporation_id,
                "Joined Alliance",
            )
            .await;
        }
        BotNotification::NotifyCorpLeftAlliance(alliance_id, corporation_id) => {
            send_corp_notification(
                ctx,
                channel_id,
                info,
                alliance_id,
                corporation_id,
                "Left Alliance",
            )
            .await;
        }
        BotNotification::NotifyAdm(adm_status) => {
            send_adm_notification(ctx, channel_id, info, adm_status).await;
        }
    };
}

pub async fn run(
    info: InformationService,
    adm_configuration: AdmConfiguration,
    adm: AdmService,
    receiver: UnboundedReceiver<BotNotification>,
    token: String,
    notification_channel_id: u64,
) -> BotResult {
    let intents = GatewayIntents::GUILD_MESSAGES;

    let bot = Bot {
        channel_id: notification_channel_id,
        adm_configuration,
        information: info,
        adm_service: adm,
        command_receiver: RwLock::new(Some(receiver)),
    };

    let mut client = Client::builder(&token, intents).event_handler(bot).await?;

    client
        .start()
        .await
        .map_err(|err| anyhow::Error::from(err))?;

    Ok(())
}
