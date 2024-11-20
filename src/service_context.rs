use envio::{EnvioManager, EnvioProject};
use gadget_sdk::config::StdGadgetConfiguration;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::process::Child;
use tokio::sync::RwLock;

use crate::{envio, generator, indexer_utils};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct SpawnIndexerParams {
    /// The indexer configuration containing contract and event details
    pub config: indexer_utils::IndexerConfig,
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
    pub config: indexer_utils::IndexerConfig,
    pub output_dir: PathBuf,
    pub process: Option<Child>,
    pub status: IndexerStatus,
}

#[derive(Debug, Clone, PartialEq)]
pub enum IndexerStatus {
    Configured,
    Starting,
    Running,
    Failed(String),
    Stopped,
}

#[derive(Clone)]
pub struct ServiceContext {
    pub config: StdGadgetConfiguration,
    pub indexers: Arc<RwLock<HashMap<String, IndexerProcess>>>,
    envio_manager: Arc<EnvioManager>,
}

impl ServiceContext {
    pub fn new(config: StdGadgetConfiguration, data_dir: PathBuf) -> Self {
        Self {
            config,
            indexers: Arc::new(RwLock::new(HashMap::new())),
            envio_manager: Arc::new(EnvioManager::new(data_dir)),
        }
    }

    fn generate_indexer_id(&self, config: &indexer_utils::IndexerConfig) -> String {
        let id = uuid::Uuid::new_v4();
        let name = config.name.to_lowercase().replace([' ', '-'], "_");
        format!("indexer_{}_{}", name, id)
    }

    pub async fn spawn_indexer(
        &self,
        config: indexer_utils::IndexerConfig,
    ) -> Result<SpawnIndexerResult, String> {
        let id = self.generate_indexer_id(&config);
        let mut indexers = self.indexers.write().await;

        if indexers.contains_key(&id) {
            return Err(format!("Indexer with id {} already exists", id));
        }

        // Initialize envio project
        let project = self.envio_manager.init_project(&id).await?;

        // Generate files into the correct envio directory structure
        let generator = generator::IndexerGenerator::new(&config, &project.dir);
        generator.generate()?;

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

    pub async fn get_indexer_config(
        &self,
        id: &str,
    ) -> Result<indexer_utils::IndexerConfig, String> {
        let indexers = self.indexers.read().await;
        let process = indexers
            .get(id)
            .ok_or_else(|| format!("Indexer {} not found", id))?;
        Ok(process.config.clone())
    }

    pub async fn update_indexer(
        &self,
        id: &str,
        new_config: indexer_utils::IndexerConfig,
    ) -> Result<(), String> {
        // First stop the indexer
        self.stop_indexer(id).await?;

        // Update configuration
        let mut indexers = self.indexers.write().await;
        let process = indexers
            .get_mut(id)
            .ok_or_else(|| format!("Indexer {} not found", id))?;

        process.config = new_config;

        // Regenerate files
        let generator = generator::IndexerGenerator::new(&process.config, &process.output_dir);
        generator.generate()?;

        // Restart the indexer
        self.start_indexer(id).await
    }
}
