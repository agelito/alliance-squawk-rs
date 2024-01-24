use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use tokio::sync::mpsc::UnboundedSender;

use crate::{bot::BotNotification, esi::EsiID};

use super::adm_service::{AdmService, Status};

const ADM_UPDATE_TIME_SECONDS: u64 = 3600;

pub struct AdmNotificationService {
    adm: AdmService,
    last_adm_update: Option<Instant>,
    notifications: UnboundedSender<BotNotification>,
    history: HashMap<EsiID, Status>,
}

impl AdmNotificationService {
    pub fn new(adm: AdmService, notifications: UnboundedSender<BotNotification>) -> Self {
        AdmNotificationService {
            adm,
            notifications,
            last_adm_update: None,
            history: Default::default(),
        }
    }

    pub async fn send_adm_notifications(&mut self) -> anyhow::Result<()> {
        self.last_adm_update = Some(Instant::now());

        let system_adms = self.adm.get_adm_status().await;

        for system_adm in system_adms {
            let prev_adm = self.history.remove(&system_adm.system_id);

            if let Err(_) = match (system_adm.status, prev_adm) {
                (Status::Warning(_), Some(Status::Good(_))) => self
                    .notifications
                    .send(BotNotification::NotifyAdm(system_adm)),
                (Status::Critical(_), Some(Status::Warning(_))) => self
                    .notifications
                    .send(BotNotification::NotifyAdm(system_adm)),
                (Status::Warning(_), None) => self
                    .notifications
                    .send(BotNotification::NotifyAdm(system_adm)),
                (Status::Critical(_), None) => self
                    .notifications
                    .send(BotNotification::NotifyAdm(system_adm)),
                (_, _) => Ok(()),
            } {
                tracing::error!(?system_adm, "couldn't send adm status to bot");

                return Err(anyhow::Error::msg("couldn't send notification to bot")
                    .context("bot not running"));
            }

            self.history.insert(system_adm.system_id, system_adm.status);
        }

        Ok(())
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        loop {
            match self.last_adm_update {
                Some(last_alliance_queue_update)
                    if last_alliance_queue_update.elapsed()
                        >= Duration::from_secs(ADM_UPDATE_TIME_SECONDS) =>
                {
                    self.send_adm_notifications().await?;
                }
                None => self.send_adm_notifications().await?,
                _ => {}
            };

            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
}
