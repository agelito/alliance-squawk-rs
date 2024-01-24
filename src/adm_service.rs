use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use tokio::sync::mpsc::UnboundedSender;

use crate::{
    bot::BotNotification,
    esi::{Esi, EsiID},
};

const ADM_UPDATE_TIME_SECONDS: u64 = 3600;
const TCU_STRUCTURE_ID: EsiID = 32226;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AdmStatus {
    Warning(f32),
    Critical(f32),
}

pub struct AdmService {
    esi: Esi,
    alliance_id: EsiID,
    include_tcus: bool,
    last_adm_update: Option<Instant>,
    adm_data: HashMap<EsiID, f32>,
    notifications: UnboundedSender<BotNotification>,
}

impl AdmService {
    pub fn new(
        esi: Esi,
        alliance_id: EsiID,
        include_tcus: bool,
        notifications: UnboundedSender<BotNotification>,
    ) -> Self {
        AdmService {
            esi,
            alliance_id,
            include_tcus,
            last_adm_update: None,
            adm_data: Default::default(),
            notifications,
        }
    }

    fn select_adm_status(
        adm: f32,
        previous_adm: Option<f32>,
        warning_threshold: f32,
        critical_threshold: f32,
    ) -> Option<AdmStatus> {
        let is_critical_state = adm <= critical_threshold;
        let is_warning_state = adm <= warning_threshold;

        if let Some(previous_adm) = previous_adm {
            let was_critical_state = previous_adm <= critical_threshold;
            let was_warning_state = previous_adm <= warning_threshold;

            match (
                was_warning_state,
                is_warning_state,
                was_critical_state,
                is_critical_state,
            ) {
                (_, _, false, true) => Some(AdmStatus::Critical(adm)),
                (false, true, _, _) => Some(AdmStatus::Warning(adm)),
                _ => None,
            }
        } else {
            match (is_warning_state, is_critical_state) {
                (true, true) => Some(AdmStatus::Critical(adm)),
                (true, false) => Some(AdmStatus::Warning(adm)),
                _ => None,
            }
        }
    }

    pub async fn update_system_adm_levels(&mut self) {
        self.last_adm_update = Some(Instant::now());

        let sovereignty_structures = self.esi.get_sovereignty_structures().await;

        if let Err(error) = &sovereignty_structures {
            tracing::error!(?error, "couldn't fetch sovereignty structures");
        }

        let sovereignty_structures: Vec<_> = sovereignty_structures
            .iter()
            .flatten()
            .filter(|sovereignty_structure| {
                sovereignty_structure.alliance_id == self.alliance_id
                    && (self.include_tcus
                        || sovereignty_structure.structure_type_id != TCU_STRUCTURE_ID)
            })
            .collect();

        tracing::debug!(
            sov_count = sovereignty_structures.len(),
            alliance_id = self.alliance_id,
            "fetched sovereignty structures"
        );

        for sov_structure in sovereignty_structures {
            if sov_structure.vulnerability_occupancy_level.is_none() {
                continue;
            }

            let adm = sov_structure.vulnerability_occupancy_level.unwrap();

            // TODO(axel): Allow configuring the threshold per system.
            let adm_warning_threshold = 3.4_f32;
            let adm_critical_threshold = 3.0_f32;

            let previous_adm = self.adm_data.remove(&sov_structure.solar_system_id);

            let adm_status = AdmService::select_adm_status(
                adm,
                previous_adm,
                adm_warning_threshold,
                adm_critical_threshold,
            );

            tracing::debug!(
                ?adm_status,
                "status for system {}",
                sov_structure.solar_system_id
            );

            if let Some(adm_status) = adm_status {
                if let Err(_) = self.notifications.send(BotNotification::NotifyAdm(
                    sov_structure.solar_system_id,
                    adm_status,
                )) {
                    tracing::error!(
                        ?adm_status,
                        system_id = sov_structure.solar_system_id,
                        "couldn't send adm status to bot"
                    )
                }
            }

            self.adm_data.insert(sov_structure.solar_system_id, adm);
        }
    }

    pub async fn run(&mut self) {
        loop {
            match self.last_adm_update {
                Some(last_alliance_queue_update)
                    if last_alliance_queue_update.elapsed()
                        >= Duration::from_secs(ADM_UPDATE_TIME_SECONDS) =>
                {
                    self.update_system_adm_levels().await
                }
                None => self.update_system_adm_levels().await,
                _ => {}
            };

            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use tracing_test::traced_test;

    use crate::adm_service::AdmStatus;

    use super::AdmService;

    #[traced_test]
    #[test]
    fn select_adm_status_critical() {
        let status = AdmService::select_adm_status(1.0, Some(1.2), 1.2, 1.0);

        assert!(status.is_some_and(|status| status == AdmStatus::Critical(1.0)));
    }

    #[traced_test]
    #[test]
    fn select_adm_status_critical_prio() {
        let status = AdmService::select_adm_status(1.0, Some(6.0), 2.0, 1.2);

        assert!(status.is_some_and(|status| status == AdmStatus::Critical(1.0)));
    }

    #[traced_test]
    #[test]
    fn select_adm_status_critical_latched() {
        let status = AdmService::select_adm_status(1.0, Some(1.0), 1.2, 1.0);

        assert!(status.is_none());
    }

    #[traced_test]
    #[test]
    fn select_adm_status_warning() {
        let status = AdmService::select_adm_status(1.2, Some(1.4), 1.2, 1.0);

        assert!(status.is_some_and(|status| status == AdmStatus::Warning(1.2)));
    }

    #[traced_test]
    #[test]
    fn select_adm_status_warning_latched() {
        let status = AdmService::select_adm_status(1.2, Some(1.2), 1.2, 1.0);

        assert!(status.is_none());
    }
}
