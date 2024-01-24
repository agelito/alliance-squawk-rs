use crate::{
    esi::{Esi, EsiID},
    services::adm_configuration::Importance,
};

use super::{adm_configuration::AdmConfiguration, information_service::InformationService};

const TCU_STRUCTURE_ID: EsiID = 32226;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Status {
    Good(f32),
    Warning(f32),
    Critical(f32),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SystemAdm {
    pub system_id: EsiID,
    pub status: Status,
}

#[derive(Clone)]
pub struct AdmService {
    esi: Esi,
    alliance_id: EsiID,
    include_tcus: bool,
    information: InformationService,
    configuration: AdmConfiguration,
}

impl AdmService {
    pub fn new(
        esi: Esi,
        alliance_id: EsiID,
        include_tcus: bool,
        information: InformationService,
        configuration: AdmConfiguration,
    ) -> AdmService {
        AdmService {
            esi,
            alliance_id,
            include_tcus,
            information,
            configuration,
        }
    }

    pub async fn get_adm_status(&self) -> Vec<SystemAdm> {
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
                    && sovereignty_structure
                        .vulnerability_occupancy_level
                        .is_some()
            })
            .collect();

        tracing::debug!(
            sov_count = sovereignty_structures.len(),
            alliance_id = self.alliance_id,
            "fetched sovereignty structures"
        );

        let mut systems = vec![];

        for sov_structure in sovereignty_structures {
            let adm = sov_structure.vulnerability_occupancy_level.unwrap();

            if let Ok(system) = self
                .information
                .get_system(sov_structure.solar_system_id)
                .await
            {
                let importance = self
                    .configuration
                    .get_importance(&system.name)
                    .await
                    .unwrap_or(Importance::Green);

                let adm_warning_threshold = importance.warning_threshold();
                let adm_critical_threshold = importance.critical_threshold();

                let status = AdmService::select_adm_status(
                    adm,
                    adm_warning_threshold,
                    adm_critical_threshold,
                );

                tracing::debug!(
                    ?status,
                    "status for system {}",
                    sov_structure.solar_system_id
                );

                systems.push(SystemAdm {
                    system_id: sov_structure.solar_system_id,
                    status,
                });
            } else {
                tracing::error!(
                    system_id = sov_structure.solar_system_id,
                    "couldn't get system"
                );
            }
        }

        systems
    }

    fn select_adm_status(adm: f32, warning_threshold: f32, critical_threshold: f32) -> Status {
        let is_critical_state = adm <= critical_threshold;
        let is_warning_state = adm <= warning_threshold;

        match (is_warning_state, is_critical_state) {
            (_, true) => Status::Critical(adm),
            (true, _) => Status::Warning(adm),
            _ => Status::Good(adm),
        }
    }
}

#[cfg(test)]
mod tests {
    use tracing_test::traced_test;

    use super::{AdmService, Status};

    #[traced_test]
    #[test]
    fn select_adm_status_critical() {
        let status = AdmService::select_adm_status(1.0, 1.2, 1.0);

        assert!(status == Status::Critical(1.0));
    }

    #[traced_test]
    #[test]
    fn select_adm_status_critical_prio() {
        let status = AdmService::select_adm_status(1.0, 2.0, 1.2);

        assert!(status == Status::Critical(1.0));
    }

    #[traced_test]
    #[test]
    fn select_adm_status_warning() {
        let status = AdmService::select_adm_status(1.2, 1.2, 1.0);

        assert!(status == Status::Warning(1.2));
    }
}
