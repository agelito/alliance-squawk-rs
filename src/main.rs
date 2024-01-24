use std::env;

use bot::BotNotification;
use esi::Esi;
use services::{
    adm_configuration::AdmConfiguration, adm_notification_service::AdmNotificationService,
    adm_service::AdmService, corporations_service::CorporationsService,
    information_service::InformationService,
};

mod bot;
mod commands;
mod esi;
mod services;

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();

    tracing_subscriber::fmt::init();

    let alliance_id = env::var("ALLIANCE_ID")
        .expect("`ALLIANCE_ID` configuration variable")
        .parse()
        .expect("`ALLIANCE_ID` is an integer");

    let token = env::var("DISCORD_TOKEN").expect("`DISCORD_TOKEN` configuration variable");

    let notify_corp_channel_id = env::var("NOTIFY_CORP_CHANNEL_ID")
        .expect("`NOTIFY_CORP_CHANNEL_ID` configuration variable")
        .parse()
        .expect("`NOTIFY_CORP_CHANNEL_ID` is a valid integer");

    let notify_adm_channel_id = env::var("NOTIFY_ADM_CHANNEL_ID")
        .expect("`NOTIFY_ADM_CHANNEL_ID` configuration variable")
        .parse()
        .expect("`NOTIFY_ADM_CHANNEL_ID` is a valid integer");

    let (notification_sender, notification_receiver) =
        tokio::sync::mpsc::unbounded_channel::<BotNotification>();

    let esi = Esi::new();
    let information_service = InformationService::new(esi.clone());

    let adm_configuration = AdmConfiguration::load_configuration()
        .await
        .expect("loading adm configuration");

    let adm_service = AdmService::new(
        esi.clone(),
        alliance_id,
        false,
        information_service.clone(),
        adm_configuration.clone(),
    );

    let mut corporation_service =
        CorporationsService::new(esi.clone(), notification_sender.clone());

    let mut adm_notification_service =
        AdmNotificationService::new(adm_service.clone(), notification_sender.clone());

    let result = tokio::try_join!(
        tokio::spawn(async move {
            if let Err(why) = bot::run(
                information_service,
                adm_configuration,
                adm_service,
                notification_receiver,
                token,
                notify_adm_channel_id,
                notify_corp_channel_id,
            )
            .await
            {
                tracing::error!(?why, "could not start bot");
            }
        }),
        tokio::spawn(async move {
            if let Err(why) = adm_notification_service.run().await {
                tracing::error!(?why, "adm service stopped");
            }
        }),
        tokio::spawn(async move {
            if let Err(why) = corporation_service.run().await {
                tracing::error!(?why, "corporation service stopped");
            }
        })
    );

    if let Err(why) = result {
        tracing::error!(?why, "exiting with error");
    }
}
