use anyhow::Context;
use reqwest::{Client, Url};
use serde::Deserialize;

pub type ApiResult<T> = Result<T, anyhow::Error>;

pub type EsiID = u64;

#[derive(Debug, Clone)]
pub struct Esi {
    client: Client,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Alliance {
    pub creator_corporation_id: EsiID,
    pub creator_id: EsiID,
    pub date_founded: String,
    pub executor_corporation_id: Option<EsiID>,
    pub faction_id: Option<EsiID>,
    pub name: String,
    pub ticker: String,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Corporation {
    pub alliance_id: Option<EsiID>,
    pub ceo_id: EsiID,
    pub creator_id: EsiID,
    pub date_founded: Option<String>,
    pub description: Option<String>,
    pub faction_id: Option<EsiID>,
    pub home_station_id: Option<EsiID>,
    pub member_count: EsiID,
    pub name: String,
    pub shares: Option<EsiID>,
    pub tax_rate: f32,
    pub ticker: String,
    pub url: Option<String>,
    pub war_eligible: Option<bool>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct SovereigntyStructure {
    pub alliance_id: EsiID,
    pub solar_system_id: EsiID,
    pub structure_id: EsiID,
    pub structure_type_id: EsiID,
    pub vulnerability_occupancy_level: Option<f32>,
    pub vulnerable_end_time: Option<String>,
    pub vulnerable_start_time: Option<String>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct System {
    pub system_id: EsiID,
    pub constellation_id: EsiID,
    pub name: String,
    pub security_status: f32,
}

const BASE_URI: &str = "https://esi.evetech.net/latest/";

fn create_endpoint_url(path: &str) -> ApiResult<Url> {
    let base_url = Url::parse(BASE_URI)?;
    let mut url = base_url.join(path)?;

    url.query_pairs_mut()
        .append_pair("datasource", "tranquility");

    Ok(url)
}

impl Esi {
    pub fn new() -> Self {
        Esi {
            client: Client::new(),
        }
    }

    pub async fn get_alliance_ids(&self) -> ApiResult<Vec<EsiID>> {
        let url = create_endpoint_url("alliances/").context("create url")?;

        tracing::debug!(?url, "fetch alliances");

        let response = self.client.get(url).send().await.context("/alliances/")?;

        let alliance_ids = response
            .json::<Vec<EsiID>>()
            .await
            .context("parse /alliances/ response")?;

        tracing::debug!(?alliance_ids, "response");

        Ok(alliance_ids)
    }

    pub async fn get_alliance(&self, alliance_id: EsiID) -> ApiResult<Alliance> {
        let resource = format!("alliances/{}/", alliance_id);
        let url = create_endpoint_url(&resource).context("create url")?;

        tracing::debug!(?url, "fetch alliance");

        let response = self.client.get(url).send().await.context("fetch alliance")?;
        let alliance = response.json::<Alliance>().await.context("parse alliance")?;

        tracing::debug!(?alliance, "response");

        Ok(alliance)
    }

    pub async fn get_alliance_corporations(&self, alliance_id: EsiID) -> ApiResult<Vec<EsiID>> {
        let resource = format!("alliances/{}/corporations/", alliance_id);
        let url = create_endpoint_url(&resource).context("create url")?;

        tracing::debug!(?url, "fetch alliance corporations");

        let response = self.client.get(url).send().await.context("fetch alliance corporations")?;
        let corporations = response.json::<Vec<EsiID>>().await.context("parse alliance corporations")?;

        tracing::debug!(?corporations, "response");

        Ok(corporations)
    }

    pub async fn get_corporation(&self, corporation_id: EsiID) -> ApiResult<Corporation> {
        let resource = format!("corporations/{}", corporation_id);
        let url = create_endpoint_url(&resource).context("create url")?;

        tracing::debug!(?url, "fetch corporation");

        let response = self.client.get(url).send().await.context("fetch corporation")?;
        let corporation = response.json::<Corporation>().await.context("parse corporation")?;

        tracing::debug!(?corporation, "response");

        Ok(corporation)
    }

    pub async fn get_system(&self, system_id: EsiID) -> ApiResult<System> {
        let resource = format!("universe/systems/{}", system_id);
        let url = create_endpoint_url(&resource).context("create url")?;

        tracing::debug!(?url, "fetch system");

        let response = self.client.get(url).send().await.context("fetch system")?;
        let system = response.json::<System>().await.context("parse system")?;

        tracing::debug!(?system, "response");

        Ok(system)
    }

    pub async fn get_sovereignty_structures(&self) -> ApiResult<Vec<SovereigntyStructure>> {
        let resource = "sovereignty/structures/";
        let url = create_endpoint_url(&resource).context("create url")?;

        tracing::debug!(?url, "fetch sovereignty structures");

        let response = self.client.get(url).send().await.context("fetch sovereignty structures")?;
        let sovereignty_structures = response.json::<Vec<SovereigntyStructure>>().await.context("parse sovereignty structures")?;

        tracing::debug!(structure_count=sovereignty_structures.len(), "response");

        Ok(sovereignty_structures)
    }
}

#[cfg(test)]
mod tests {
    use tracing_test::traced_test;

    use super::Esi;

    #[traced_test]
    #[tokio::test]
    async fn get_alliances() {
        let esi = Esi::new();
        let alliances = esi.get_alliance_ids().await.unwrap();

        assert!(!alliances.is_empty());
    }

    #[traced_test]
    #[tokio::test]
    async fn get_alliance() {
        let esi = Esi::new();
        let alliance = esi.get_alliance(99010468).await.unwrap();

        assert!(alliance.name.contains("Weapons Of Mass Production."));
    }

    #[traced_test]
    #[tokio::test]
    async fn get_alliance_corporations() {
        let esi = Esi::new();
        let corporations = esi.get_alliance_corporations(99010468).await.unwrap();

        assert!(!corporations.is_empty());
    }

    #[traced_test]
    #[tokio::test]
    async fn get_corporation() {
        let esi = Esi::new();
        let corporation = esi.get_corporation(98633922).await.unwrap();

        assert!(corporation.name.contains("Guns-R-Us Toy Company"));
    }

    #[traced_test]
    #[tokio::test]
    async fn get_sovereignty_structures() {
        let esi = Esi::new();
        let sovereignty_structures = esi.get_sovereignty_structures().await.unwrap();

        assert!(!sovereignty_structures.is_empty());
    }
}
