use std::env;
use std::sync::Arc;

use anyhow::Context as _;
use serenity::all::ChannelId;
use serenity::async_trait;
use serenity::builder::{CreateEmbed, CreateMessage, CreateEmbedFooter};
use serenity::model::gateway::Ready;
use serenity::prelude::*;
use tokio::sync::mpsc::UnboundedReceiver;

use crate::adm_service::AdmStatus;
use crate::esi::EsiID;
use crate::information_service::InformationService;

#[allow(dead_code)]
pub enum BotNotification {
    NotifyCorpJoinAlliance(EsiID, EsiID),
    NotifyCorpLeftAlliance(EsiID, EsiID),
    NotifyAdm(EsiID, AdmStatus),
}

pub type BotResult = anyhow::Result<()>;

struct Bot {
    channel_id: u64,
    information: RwLock<Option<InformationService>>,
    command_receiver: RwLock<Option<UnboundedReceiver<BotNotification>>>,
}

#[async_trait]
impl EventHandler for Bot {
    async fn ready(&self, ctx: Context, ready: Ready) {
        tracing::info!(bot_name = ready.user.name, "connected");

        if let Some(mut receiver) = self.command_receiver.write().await.take() {
            let information = self
                .information
                .write()
                .await
                .take()
                .expect("information service value");

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
    system_id: EsiID,
    status: AdmStatus,
) {
    tracing::info!(?status, ?system_id, "send adm notification");

    match info.get_system(system_id).await {
        Ok(system) => {
            let (msg, footer, adm, color) = match status {
                AdmStatus::Warning(adm) => (format!("{} ADM is deteriorated!", system.name), "Please do some ratting or mining here.", adm, (238, 210, 2)),
                AdmStatus::Critical(adm) => (format!("{} ADM is critically low!", system.name), "Do ratting or mining here ASAP!!!", adm, (255, 103, 0)),
            };

            let system_link = format!("https://evemaps.dotlan.net/system/{}", system.name);

            let embed = CreateEmbed::new()
                .title(msg)
                .field(
                    "System",
                    format!("[{}]({})", system.name, system_link),
                    false,
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
        BotNotification::NotifyAdm(system_id, adm_status) => {
            send_adm_notification(ctx, channel_id, info, system_id, adm_status).await;
        }
    };
}

pub async fn run(
    info: InformationService,
    receiver: UnboundedReceiver<BotNotification>,
) -> BotResult {
    let token = env::var("DISCORD_TOKEN")
        .map_err(|_| anyhow::Error::msg("missing `DISCORD_TOKEN` configuration variable"))
        .context("configuration")?;

    let channel_id = env::var("NOTIFY_CHANNEL_ID")
        .map_err(|_| anyhow::Error::msg("missing `NOTIFY_CHANNEL_ID` configuration variable"))
        .and_then(|channel_id| {
            channel_id.parse::<u64>().map_err(|_| {
                anyhow::Error::msg("value in `NOTIFY_CHANNEL_ID` is not a valid integer")
            })
        })
        .context("configuration")?;

    let intents = GatewayIntents::GUILD_MESSAGES;

    let bot = Bot {
        channel_id,
        information: RwLock::new(Some(info)),
        command_receiver: RwLock::new(Some(receiver)),
    };

    let mut client = Client::builder(&token, intents)
        .event_handler(bot)
        .await
        .context("create client")?;

    client
        .start()
        .await
        .map_err(|err| anyhow::Error::from(err))
        .context("start client")?;

    Ok(())
}
