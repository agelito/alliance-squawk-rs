use std::collections::HashMap;

use tokio::sync::RwLock;

use crate::esi::{Alliance, Corporation, Esi};

#[derive(Debug)]
pub struct InformationService {
    esi: Esi,
    alliances: RwLock<HashMap<i32, Alliance>>,
    corporations: RwLock<HashMap<i32, Corporation>>,
}

impl InformationService {
    pub fn new(esi: Esi) -> Self {
        InformationService {
            esi,
            alliances: Default::default(),
            corporations: Default::default(),
        }
    }

    pub async fn get_alliance(&self, id: i32) -> anyhow::Result<Alliance> {
        // TODO(axel): Only get write lock if actually having to write value
        let mut alliances = self.alliances.write().await;

        if let Some(alliance) = alliances.get(&id) {
            Ok(alliance.clone())
        } else {
            let alliance = self.esi.get_alliance(id).await?;

            alliances.insert(id, alliance.clone());

            Ok(alliance)
        }
    }

    pub async fn get_corporation(&self, id: i32) -> anyhow::Result<Corporation> {
        // TODO(axel): Only get write lock if actually having to write value
        let mut corporations = self.corporations.write().await;

        if let Some(corporation) = corporations.get(&id) {
            Ok(corporation.clone())
        } else {
            let corporation = self.esi.get_corporation(id).await?;

            corporations.insert(id, corporation.clone());

            Ok(corporation)
        }
    }
}
