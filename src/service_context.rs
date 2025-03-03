use crate::envio_utils::project::IndexerProgress;
use crate::envio_utils::project::IndexerStatus;
use crate::envio_utils::{self, EnvioManager, EnvioProject, IndexerConfig, IndexerLogMessage};
use blueprint_sdk::config::GadgetConfiguration;
use blueprint_sdk::macros::contexts::ServicesContext;
use blueprint_sdk::macros::contexts::TangleClientContext;
use blueprint_sdk::std::collections::HashMap;
use blueprint_sdk::std::path::PathBuf;
use blueprint_sdk::std::sync::Arc;
use blueprint_sdk::tokio;
use blueprint_sdk::tokio::process::Child;
use blueprint_sdk::tokio::sync::RwLock;
use schemars::JsonSchema;

use blueprint_sdk::tokio::sync::mpsc;
use serde::{Deserialize, Serialize};
use std::fmt;

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
    pub logs: Vec<String>,
    pub last_checked: std::time::Instant,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub enum DeploymentMode {
    Local,
}

#[derive(Clone, ServicesContext, TangleClientContext)]
pub struct ServiceContext {
    #[config]
    pub config: GadgetConfiguration,
    #[call_id]
    pub call_id: Option<u64>,
    pub indexers: Arc<RwLock<HashMap<String, IndexerProcess>>>,
    pub envio_manager: Arc<EnvioManager>,
    pub deployment_mode: DeploymentMode,
}

impl ServiceContext {
    pub fn new(config: GadgetConfiguration, data_dir: PathBuf) -> Self {
        Self {
            config,
            call_id: None,
            indexers: Arc::new(RwLock::new(HashMap::new())),
            envio_manager: Arc::new(EnvioManager::new(data_dir)),
            deployment_mode: DeploymentMode::Local,
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

        // Create indexer process entry with new fields
        let process = IndexerProcess {
            id: id.clone(),
            config: config.clone(),
            output_dir: project.dir,
            process: None,
            status: IndexerStatus::Configured,
            logs: vec![format!("[{}] Indexer created", chrono::Local::now())],
            last_checked: std::time::Instant::now(),
        };

        indexers.insert(id.clone(), process);
        Ok(SpawnIndexerResult {
            id,
            message: "Indexer spawned successfully".to_string(),
        })
    }

    pub async fn start_indexer(&self, id: &str) -> Result<SpawnIndexerResult, String> {
        let mut indexers = self.indexers.write().await;
        let process = indexers
            .get_mut(id)
            .ok_or_else(|| format!("Indexer {} not found", id))?;

        println!("Starting indexer {}", id);
        process.status = IndexerStatus::Starting;

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

        // Start the indexer
        let start_result = self.envio_manager.start_dev(&mut project).await;
        if let Err(e) = start_result {
            process.status = IndexerStatus::Failed(e.to_string());
            return Err(format!("Failed to start indexer: {}", e));
        }

        process.process = project.process;
        process.last_checked = std::time::Instant::now();
        process
            .logs
            .push(format!("[{}] Indexer started", chrono::Local::now()));

        // Update status to starting - we'll check health separately
        process.status = IndexerStatus::Starting;

        Ok(SpawnIndexerResult {
            id: id.to_string(),
            message: "Indexer started successfully".to_string(),
        })
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

        let stop_result = self.envio_manager.stop_dev(&mut project).await;

        if let Err(e) = stop_result {
            process.logs.push(format!(
                "[{}] Error stopping indexer: {}",
                chrono::Local::now(),
                e
            ));
            // Still mark as stopped even if we had an error
        }

        process
            .logs
            .push(format!("[{}] Indexer stopped", chrono::Local::now()));
        process.status = IndexerStatus::Stopped;

        Ok(())
    }

    pub async fn monitor_indexer(&self, id: &str) -> Result<IndexerStatus, String> {
        let mut indexers = self.indexers.write().await;
        let process = indexers
            .get_mut(id)
            .ok_or_else(|| format!("Indexer {} not found", id))?;

        // Check status based on stored status enum variants
        match process.status {
            IndexerStatus::Starting | IndexerStatus::Running => {
                // Only check status every few seconds to avoid too much overhead
                let elapsed = process.last_checked.elapsed();
                if elapsed > std::time::Duration::from_secs(5) {
                    // Create a temporary EnvioProject with the current process
                    let mut project = EnvioProject {
                        id: id.to_string(),
                        dir: process.output_dir.clone(),
                        process: None,
                    };

                    // Move the process out temporarily to avoid clone issues
                    if let Some(child_process) = process.process.take() {
                        project.process = Some(child_process);

                        // Monitor using EnvioManager
                        match self.envio_manager.monitor_indexer(&project).await {
                            Ok(new_status) => {
                                // Update status
                                process.status = new_status;

                                // Add log entry
                                let status_str: String = From::from(process.status.clone());
                                process.logs.push(format!(
                                    "[{}] Status updated: {}",
                                    chrono::Local::now(),
                                    status_str
                                ));
                            }
                            Err(e) => {
                                process.logs.push(format!(
                                    "[{}] Error monitoring indexer: {}",
                                    chrono::Local::now(),
                                    e
                                ));
                            }
                        }

                        // Move the process back
                        process.process = project.process;
                    }

                    process.last_checked = std::time::Instant::now();
                }
            }
            _ => {} // No need to update for other statuses
        };

        // Return a copy of the status
        Ok(process.status.clone())
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

    // Getter methods for internal components
    pub fn get_envio_manager(&self) -> &Arc<EnvioManager> {
        &self.envio_manager
    }

    pub fn get_indexers(&self) -> &Arc<RwLock<HashMap<String, IndexerProcess>>> {
        &self.indexers
    }

    /// Create a new service context for testing
    pub async fn new_test() -> Self {
        let config = GadgetConfiguration::default();
        let test_dir = std::env::temp_dir().join(format!("envio_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&test_dir).expect("Failed to create test directory");

        println!("Created test directory at: {:?}", test_dir);
        Self::new(config, test_dir)
    }

    /// Subscribe to logs from a specific indexer
    pub async fn subscribe_to_indexer_logs(
        &self,
        id: &str,
    ) -> Result<mpsc::Receiver<IndexerLogMessage>, String> {
        let mut indexers = self.indexers.write().await;
        let process = indexers
            .get_mut(id)
            .ok_or_else(|| format!("Indexer {} not found", id))?;

        // Create a temporary EnvioProject from the IndexerProcess
        let mut project = EnvioProject {
            id: process.id.clone(),
            dir: process.output_dir.clone(),
            process: process.process.take(),
        };

        // Subscribe to logs
        let logs_rx = self
            .envio_manager
            .subscribe_to_logs(&mut project)
            .map_err(|e| format!("Failed to subscribe to logs: {}", e))?;

        // Move the process back
        process.process = project.process;

        Ok(logs_rx)
    }

    /// Subscribe to filtered logs from a specific indexer
    /// This provides a cleaned-up version of the log stream with duplicates and noise removed
    pub async fn subscribe_to_filtered_logs(
        &self,
        id: &str,
    ) -> Result<mpsc::Receiver<String>, String> {
        // Get the raw log stream
        let mut raw_logs = self.subscribe_to_indexer_logs(id).await?;

        // Create a new channel for the filtered logs
        let (tx, rx) = mpsc::channel::<String>(100);

        // Spawn a task to filter the logs
        tokio::spawn(async move {
            // Track previously seen lines to avoid duplicates
            let mut seen_lines = std::collections::HashSet::new();
            // Skip logo after seeing it once
            let mut shown_logo = false;
            // Last progress information for summarizing
            let mut last_progress: Option<IndexerProgress> = None;
            // Timestamp of last progress update
            let mut last_progress_time = std::time::Instant::now();

            while let Some(msg) = raw_logs.recv().await {
                match msg {
                    IndexerLogMessage::Stdout(line) => {
                        // Skip empty lines
                        if line.trim().is_empty() {
                            continue;
                        }

                        // Skip the ASCII art logo after showing it once
                        if line.contains("███████╗")
                            || line.contains("██╔════╝")
                            || line.contains("█████╗")
                            || line.contains("██╔══╝")
                            || line.contains("╚══════╝")
                        {
                            if !shown_logo && line.contains("███████╗") {
                                let _ = tx.send("[Indexer Logo displayed]".into()).await;
                                shown_logo = true;
                            }
                            continue;
                        }

                        // Only show unique lines or important status updates
                        let always_show = line.contains("Events Processed:")
                            || line.contains("Sync Time ETA:")
                            || line.contains("GraphQL:")
                            || line.contains("Chain ID:");

                        if always_show || !seen_lines.contains(&line) {
                            let _ = tx.send(line.clone()).await;
                            seen_lines.insert(line);
                        }
                    }
                    IndexerLogMessage::Stderr(line) => {
                        // Always show error messages
                        let _ = tx.send(format!("ERROR: {}", line)).await;
                    }
                    IndexerLogMessage::Progress(progress) => {
                        let events_processed = progress.clone().events_processed;
                        let blocks_current = progress.clone().blocks_current;
                        let blocks_total = progress.clone().blocks_total;
                        let chain_id = progress.clone().chain_id;
                        let percentage = progress.clone().percentage;
                        let eta = progress.clone().eta;

                        // Only send progress updates periodically or on significant changes
                        let now = std::time::Instant::now();
                        let time_to_update = now.duration_since(last_progress_time)
                            > std::time::Duration::from_secs(5);

                        let significant_change = if let Some(last) = &last_progress {
                            events_processed != last.events_processed
                                || chain_id != last.chain_id
                                || eta != last.eta
                                || (percentage.is_some() && last.percentage.is_some() && {
                                    // Calculate absolute difference without using .abs()
                                    let curr = percentage.unwrap_or(0);
                                    let prev = last.percentage.unwrap_or(0);
                                    let diff = if curr > prev {
                                        curr - prev
                                    } else {
                                        prev - curr
                                    };
                                    diff >= 5
                                })
                        } else {
                            true
                        };

                        if time_to_update || significant_change {
                            let progress_msg = format!(
                                "PROGRESS: Events: {}, Blocks: {}/{}, Chain: {}, {}%, ETA: {}",
                                events_processed.unwrap_or(0),
                                blocks_current.unwrap_or(0),
                                blocks_total.unwrap_or(0),
                                chain_id.unwrap_or_else(|| "unknown".to_string()),
                                percentage.unwrap_or(0),
                                eta.unwrap_or_else(|| "unknown".to_string())
                            );
                            let _ = tx.send(progress_msg).await;
                            last_progress = Some(progress);
                            last_progress_time = now;
                        }
                    }
                }
            }
        });

        Ok(rx)
    }
}
