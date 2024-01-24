use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};
use tokio::{fs, sync::RwLock};

const CONFIGURATION_FILE: &'static str = "adm.toml";

#[derive(Clone, Copy, Serialize, Deserialize)]
pub enum Importance {
    Red,
    Yellow,
    Green,
}

impl Importance {
    pub fn warning_threshold(&self) -> f32 {
        match self {
            Importance::Red => 4.2,
            Importance::Yellow => 3.2,
            Importance::Green => 1.2,
        }
    }

    pub fn critical_threshold(&self) -> f32 {
        match self {
            Importance::Red => 4.0,
            Importance::Yellow => 3.0,
            Importance::Green => 1.0,
        }
    }
}

impl std::fmt::Display for Importance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Importance::Red => write!(f, "Red (4.0 - 4.2)"),
            Importance::Yellow => write!(f, "Yellow (3.0 - 3.2)"),
            Importance::Green => write!(f, "Green (1.0 - 1.2"),
        }
    }
}

#[derive(Default, Serialize, Deserialize)]
struct Configuration {
    importance: HashMap<String, Importance>,
}

#[derive(Clone)]
pub struct AdmConfiguration {
    config: Arc<RwLock<Configuration>>,
}

impl AdmConfiguration {
    pub async fn load_configuration() -> anyhow::Result<AdmConfiguration> {
        let configuration = if let Ok(toml_data) = fs::read_to_string(CONFIGURATION_FILE).await {
            toml::from_str(&toml_data)?
        } else {
            Default::default()
        };

        Ok(AdmConfiguration {
            config: Arc::new(RwLock::new(configuration)),
        })
    }

    async fn save_configuration(&self, configuration: &Configuration) -> anyhow::Result<()> {
        let toml_data = toml::to_string(configuration)?;

        fs::write(CONFIGURATION_FILE, toml_data).await?;

        Ok(())
    }

    pub async fn set_importance(
        &self,
        system_name: &str,
        importance: Importance,
    ) -> anyhow::Result<()> {
        let mut config = self.config.write().await;

        config
            .importance
            .insert(system_name.to_string(), importance);

        self.save_configuration(&config).await
    }

    pub async fn get_importance(&self, system_name: &str) -> Option<Importance> {
        self.config
            .read()
            .await
            .importance
            .get(system_name)
            .copied()
    }
}
