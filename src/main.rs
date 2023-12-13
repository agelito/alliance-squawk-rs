use bot::BotCommand;
use corporations_service::ServiceEvent;
use esi::Esi;
use information_service::InformationService;

mod bot;
mod corporations_service;
mod esi;
mod information_service;

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();

    tracing_subscriber::fmt::init();

    let (bot_sender, bot_receiver) = tokio::sync::mpsc::unbounded_channel::<BotCommand>();

    let information_service = InformationService::new(Esi::new());

    let _bot_service = tokio::spawn(async move {
        bot::run(information_service, bot_receiver).await;
    });

    let (service_sender, mut service_receiver) =
        tokio::sync::mpsc::unbounded_channel::<ServiceEvent>();

    let mut service = corporations_service::CorporationsService::new(Esi::new(), service_sender);

    let _corp_service = tokio::spawn(async move {
        service.run().await;
    });

    loop {
        let service_event = service_receiver.recv().await;

        match service_event {
            Some(ServiceEvent::JoinAlliance(alliance_id, corporation_id)) => bot_sender
                .send(BotCommand::NotifyCorpJoinAlliance(
                    alliance_id,
                    corporation_id,
                ))
                .expect("bot channel is open"),
            Some(ServiceEvent::LeftAlliance(alliance_id, corporation_id)) => bot_sender
                .send(BotCommand::NotifyCorpLeftAlliance(
                    alliance_id,
                    corporation_id,
                ))
                .expect("bot channel is open"),
            None => {}
        }
    }
}
