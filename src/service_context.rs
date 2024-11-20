use envio::{EnvioManager, EnvioProject};
use gadget_sdk::config::StdGadgetConfiguration;
use schemars::JsonSchema;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::process::Child;
use tokio::sync::RwLock;

use crate::{envio, kubernetes::K8sManager};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexerConfig {
    pub name: String,
    pub abi: String,
}

impl IndexerConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.name.is_empty() {
            return Err("Indexer name cannot be empty".to_string());
        }
        if self.abi.is_empty() {
            return Err("Contract ABI cannot be empty".to_string());
        }
        // Validate ABI is valid JSON
        serde_json::from_str::<serde_json::Value>(&self.abi)
            .map_err(|e| format!("Invalid ABI JSON: {}", e))?;
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SpawnIndexerParams {
    pub config: IndexerConfig,
    pub blockchain: String,
    pub rpc_url: Option<String>,
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

#[derive(Clone)]
pub struct ServiceContext {
    pub config: StdGadgetConfiguration,
    pub indexers: Arc<RwLock<HashMap<String, IndexerProcess>>>,
    pub envio_manager: Arc<EnvioManager>,
    pub deployment_mode: DeploymentMode,
    pub k8s_manager: Option<K8sManager>,
}

impl ServiceContext {
    pub fn new(config: StdGadgetConfiguration, data_dir: PathBuf) -> Self {
        Self {
            config,
            indexers: Arc::new(RwLock::new(HashMap::new())),
            envio_manager: Arc::new(EnvioManager::new(data_dir)),
            deployment_mode: DeploymentMode::Local,
            k8s_manager: None,
        }
    }

    fn generate_indexer_id(&self, config: &IndexerConfig) -> String {
        let id = uuid::Uuid::new_v4();
        let name = config.name.to_lowercase().replace([' ', '-'], "_");
        format!("indexer_{}_{}", name, id)
    }

    pub async fn spawn_indexer(
        &self,
        config: IndexerConfig,
        blockchain: String,
        rpc_url: Option<String>,
    ) -> Result<SpawnIndexerResult, String> {
        let id = self.generate_indexer_id(&config);
        let mut indexers = self.indexers.write().await;

        if indexers.contains_key(&id) {
            return Err(format!("Indexer with id {} already exists", id));
        }

        // Initialize envio project
        let project = self
            .envio_manager
            .init_project(
                &id,
                &config.abi,
                &config.name,
                &blockchain,
                rpc_url.as_deref(),
            )
            .await?;

        // Create indexer process entry
        let process = IndexerProcess {
            id: id.clone(),
            config,
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
