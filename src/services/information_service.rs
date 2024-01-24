use std::{collections::HashMap, sync::Arc};

use tokio::sync::RwLock;

use crate::esi::{Alliance, Corporation, Esi, EsiID, System};

#[derive(Debug, Clone)]
pub struct InformationService {
    esi: Esi,
    alliances: Arc<RwLock<HashMap<EsiID, Alliance>>>,
    corporations: Arc<RwLock<HashMap<EsiID, Corporation>>>,
    systems: Arc<RwLock<HashMap<EsiID, System>>>,
}

impl InformationService {
    pub fn new(esi: Esi) -> Self {
        InformationService {
            esi,
            alliances: Default::default(),
            corporations: Default::default(),
            systems: Default::default(),
        }
    }

    pub async fn get_alliance(&self, id: EsiID) -> anyhow::Result<Alliance> {
        let mut alliances = self.alliances.write().await;

        if let Some(alliance) = alliances.get(&id) {
            Ok(alliance.clone())
        } else {
            let alliance = self.esi.get_alliance(id).await?;

            alliances.insert(id, alliance.clone());

            Ok(alliance)
        }
    }

    pub async fn get_corporation(&self, id: EsiID) -> anyhow::Result<Corporation> {
        let mut corporations = self.corporations.write().await;

        if let Some(corporation) = corporations.get(&id) {
            Ok(corporation.clone())
        } else {
            let corporation = self.esi.get_corporation(id).await?;

            corporations.insert(id, corporation.clone());

            Ok(corporation)
        }
    }

    pub async fn get_system(&self, id: EsiID) -> anyhow::Result<System> {
        let mut systems = self.systems.write().await;

        if let Some(system) = systems.get(&id) {
            Ok(system.clone())
        } else {
            let system = self.esi.get_system(id).await?;

            systems.insert(id, system.clone());

            Ok(system)
        }
    }
}
