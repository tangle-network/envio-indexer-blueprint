use blueprint_sdk::macros::contexts::TangleClientContext;
use blueprint_sdk::std::collections::HashMap;
use blueprint_sdk::std::path::PathBuf;
use blueprint_sdk::std::sync::Arc;
use blueprint_sdk::tokio::process::Child;
use blueprint_sdk::tokio::sync::RwLock;
use blueprint_sdk::{config::StdGadgetConfiguration, macros::contexts::ServicesContext};
use envio_utils::{EnvioManager, EnvioProject};
use schemars::JsonSchema;

use crate::{
    envio_utils::{self, IndexerConfig},
    kubernetes::K8sManager,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct SpawnIndexerParams {
    pub config: IndexerConfig,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SpawnIndexerResult {
    /// The unique ID assigned to this indexer instance
    pub id: String,
    /// Status message
    pub message: String,
}

pub struct IndexerProcess {
    pub id: String,
    pub config: IndexerConfig,
    pub output_dir: PathBuf,
    pub process: Option<Child>,
    pub status: IndexerStatus,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub enum IndexerStatus {
    Configured,
    Starting,
    Running,
    Failed(String),
    Stopped,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DeploymentMode {
    Local,
    Kubernetes,
}

#[derive(Clone, ServicesContext, TangleClientContext)]
pub struct ServiceContext {
    #[config]
    pub config: StdGadgetConfiguration,
    #[call_id]
    pub call_id: Option<u64>,
    pub indexers: Arc<RwLock<HashMap<String, IndexerProcess>>>,
    pub envio_manager: Arc<EnvioManager>,
    pub deployment_mode: DeploymentMode,
    pub k8s_manager: Option<K8sManager>,
}

impl ServiceContext {
    pub fn new(config: StdGadgetConfiguration, data_dir: PathBuf) -> Self {
        Self {
            config,
            call_id: None,
            indexers: Arc::new(RwLock::new(HashMap::new())),
            envio_manager: Arc::new(EnvioManager::new(data_dir)),
            deployment_mode: DeploymentMode::Local,
            k8s_manager: None,
        }
    }

    fn generate_indexer_id(&self, name: &str) -> String {
        let id = uuid::Uuid::new_v4();
        let name = name.to_lowercase().replace([' ', '-'], "_");
        format!("indexer_{}_{}", name, id)
    }

    pub async fn spawn_indexer(&self, config: IndexerConfig) -> Result<SpawnIndexerResult, String> {
        let id = self.generate_indexer_id(&config.name);
        let mut indexers = self.indexers.write().await;

        if indexers.contains_key(&id) {
            return Err(format!("Indexer with id {} already exists", id));
        }

        // Initialize envio project with all contracts
        let project = self
            .envio_manager
            .init_project(&id, config.clone().contracts)
            .await
            .map_err(|e| e.to_string())?;

        // Create indexer process entry
        let process = IndexerProcess {
            id: id.clone(),
            config: config.clone(),
            output_dir: project.dir,
            process: None,
            status: IndexerStatus::Configured,
        };

        indexers.insert(id.clone(), process);
        Ok(SpawnIndexerResult {
            id,
            message: "Indexer spawned successfully".to_string(),
        })
    }

    pub async fn start_indexer(&self, id: &str) -> Result<(), String> {
        let mut indexers = self.indexers.write().await;
        let process = indexers
            .get_mut(id)
            .ok_or_else(|| format!("Indexer {} not found", id))?;

        // Run codegen
        self.envio_manager
            .run_codegen(&EnvioProject {
                id: id.to_string(),
                dir: process.output_dir.clone(),
                process: None,
            })
            .await?;

        // Start dev mode
        let mut project = EnvioProject {
            id: id.to_string(),
            dir: process.output_dir.clone(),
            process: None,
        };
        self.envio_manager.start_dev(&mut project).await?;

        process.process = project.process;
        process.status = IndexerStatus::Running;

        Ok(())
    }

    pub async fn stop_indexer(&self, id: &str) -> Result<(), String> {
        let mut indexers = self.indexers.write().await;
        let process = indexers
            .get_mut(id)
            .ok_or_else(|| format!("Indexer {} not found", id))?;

        let mut project = EnvioProject {
            id: id.to_string(),
            dir: process.output_dir.clone(),
            process: process.process.take(),
        };

        self.envio_manager.stop_dev(&mut project).await?;
        process.status = IndexerStatus::Stopped;

        Ok(())
    }

    pub async fn list_indexers(&self) -> Vec<String> {
        let indexers = self.indexers.read().await;
        indexers.keys().cloned().collect()
    }

    pub async fn get_indexer_status(&self, id: &str) -> Result<IndexerStatus, String> {
        let indexers = self.indexers.read().await;
        let process = indexers
            .get(id)
            .ok_or_else(|| format!("Indexer {} not found", id))?;
        Ok(process.status.clone())
    }

    pub async fn get_indexer_config(&self, id: &str) -> Result<IndexerConfig, String> {
        let indexers = self.indexers.read().await;
        let process = indexers
            .get(id)
            .ok_or_else(|| format!("Indexer {} not found", id))?;
        Ok(process.config.clone())
    }
}
