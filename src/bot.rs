use std::env;
use std::sync::Arc;

use serenity::all::ChannelId;
use serenity::async_trait;
use serenity::builder::{CreateEmbed, CreateMessage};
use serenity::model::gateway::Ready;
use serenity::prelude::*;
use tokio::sync::mpsc::UnboundedReceiver;

use crate::information_service::InformationService;

pub enum BotCommand {
    NotifyCorpJoinAlliance(i32, i32),
    NotifyCorpLeftAlliance(i32, i32),
}

struct Bot {
    channel_id: u64,
    information: RwLock<Option<InformationService>>,
    command_receiver: RwLock<Option<UnboundedReceiver<BotCommand>>>,
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

async fn send_notification(
    ctx: &Context,
    channel_id: u64,
    info: &InformationService,
    command: BotCommand,
) {
    let (alliance_id, corporation_id, msg) = match command {
        BotCommand::NotifyCorpJoinAlliance(alliance_id, corporation_id) => {
            (alliance_id, corporation_id, "Joined Alliance")
        }
        BotCommand::NotifyCorpLeftAlliance(alliance_id, corporation_id) => {
            (alliance_id, corporation_id, "Left Alliance")
        }
    };

    tracing::info!(alliance_id, corporation_id, "send leave notification");

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

pub async fn run(info: InformationService, receiver: UnboundedReceiver<BotCommand>) {
    let token = env::var("DISCORD_TOKEN").expect("token in `DISCORD_TOKEN` environment variable");
    let channel_id = env::var("NOTIFY_CHANNEL_ID")
        .expect("channel id in `NOTIFY_CHANNEL_ID` environment variable")
        .parse::<u64>()
        .expect("channel is a valid integer");
    let intents = GatewayIntents::GUILD_MESSAGES;

    let bot = Bot {
        channel_id,
        information: RwLock::new(Some(info)),
        command_receiver: RwLock::new(Some(receiver)),
    };

    let mut client = Client::builder(&token, intents)
        .event_handler(bot)
        .await
        .expect("create client");

    if let Err(why) = client.start().await {
        tracing::error!(?why, "client error");
    }
}
