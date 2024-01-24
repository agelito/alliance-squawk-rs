use adm_service::AdmService;
use bot::BotNotification;
use corporations_service::CorporationServiceEvent;
use esi::Esi;
use information_service::InformationService;

mod adm_service;
mod bot;
mod corporations_service;
mod esi;
mod information_service;

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();

    tracing_subscriber::fmt::init();

    let (bot_sender, bot_receiver) = tokio::sync::mpsc::unbounded_channel::<BotNotification>();

    let esi = Esi::new();
    let information_service = InformationService::new(esi.clone());

    let _bot_service = tokio::spawn(async move {
        if let Err(error) = bot::run(information_service, bot_receiver).await {
            tracing::error!(?error, "could not start bot");
        };
    });

    let (service_sender, mut service_receiver) =
        tokio::sync::mpsc::unbounded_channel::<CorporationServiceEvent>();

    let mut corporation_service =
        corporations_service::CorporationsService::new(esi.clone(), service_sender.clone());

    let alliance_id = 99010468;

    let mut adm_service = AdmService::new(esi.clone(), alliance_id, false, bot_sender.clone());

    let _adm_service = tokio::spawn(async move {
        adm_service.run().await;
    });

    let _corp_service = tokio::spawn(async move {
        corporation_service.run().await;
    });

    loop {
        let service_event = service_receiver.recv().await;

        if let Some(CorporationServiceEvent::LeftAlliance(alliance_id, corporation_id)) =
            service_event
        {
            bot_sender
                .send(BotNotification::NotifyCorpLeftAlliance(
                    alliance_id,
                    corporation_id,
                ))
                .expect("bot channel is open");
        }
    }
}
