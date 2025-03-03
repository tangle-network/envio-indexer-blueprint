use super::config::{ContractConfig, ContractSource};
use anyhow::Result;
use blueprint_sdk::std::path::PathBuf;
use blueprint_sdk::tokio;
use blueprint_sdk::tokio::process::{Child, Command};
use blueprint_sdk::tokio::sync::mpsc;
use rexpect::spawn;
use std::io::BufReader;
use std::sync::Arc;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum EnvioError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Failed to capture process output: {0}")]
    ProcessOutput(String),
    #[error("Process failed: {0}")]
    ProcessFailed(String),
    #[error("Invalid state: {0}")]
    InvalidState(String),
    #[error("Docker error: {0}")]
    DockerError(String),
    #[error("Reqwest error: {0}")]
    ReqwestError(#[from] reqwest::Error),
    #[error("Serde JSON error: {0}")]
    SerdeJsonError(#[from] serde_json::Error),
    #[error("Join error: {0}")]
    JoinError(#[from] blueprint_sdk::tokio::task::JoinError),
    #[error("rexpect error: {0}")]
    RexpectError(#[from] rexpect::error::Error),
}

impl From<EnvioError> for String {
    fn from(error: EnvioError) -> Self {
        error.to_string()
    }
}

pub struct EnvioManager {
    base_dir: PathBuf,
}

#[derive(Debug)]
pub struct EnvioProject {
    pub id: String,
    pub dir: PathBuf,
    pub process: Option<Child>,
}

impl EnvioManager {
    pub fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    pub async fn run_codegen(&self, project: &EnvioProject) -> Result<(), EnvioError> {
        // Verify config.yaml exists
        let config_path = project.dir.join("config.yaml");
        if !config_path.exists() {
            return Err(EnvioError::InvalidState(
                "config.yaml not found. Project may not be properly initialized".into(),
            ));
        }

        // Ensure we're in the project directory
        std::env::set_current_dir(&project.dir)?;

        let status = Command::new("envio")
            .arg("codegen")
            .current_dir(&project.dir) // Belt and suspenders approach
            .status()
            .await?;

        if !status.success() {
            return Err(EnvioError::ProcessFailed("Codegen failed".into()));
        }

        Ok(())
    }

    pub async fn start_dev(&self, project: &mut EnvioProject) -> Result<(), EnvioError> {
        if project.process.is_some() {
            return Err(EnvioError::InvalidState(
                "Project already has a running process".into(),
            ));
        }

        // Spawn the process with piped output so we can capture logs
        let child = Command::new("envio")
            .arg("dev")
            .current_dir(&project.dir)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;

        // Store the process
        project.process = Some(child);

        // Wait a brief moment for process to start
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        // Verify process is running
        if let Some(child) = project.process.as_mut() {
            // try_wait requires a mutable reference in tokio
            match child.try_wait() {
                Ok(Some(status)) => {
                    if !status.success() {
                        return Err(EnvioError::ProcessFailed(format!(
                            "Indexer process exited immediately with status: {:?}",
                            status
                        )));
                    }
                }
                Err(e) => {
                    return Err(EnvioError::Io(e));
                }
                _ => {} // Process still running
            }
        }

        Ok(())
    }

    pub async fn stop_dev(&self, project: &mut EnvioProject) -> Result<(), EnvioError> {
        if let Some(mut child) = project.process.take() {
            println!("Stopping indexer process...");

            // First try to use envio stop command
            let stop_result = Command::new("envio")
                .arg("stop")
                .current_dir(&project.dir)
                .status()
                .await;

            // Regardless of stop command result, ensure process is terminated
            let kill_result = child.kill().await;

            if let Err(e) = kill_result {
                println!("Warning: Failed to kill process: {}", e);

                // Kill by process ID as a fallback (if we can get it)
                // The method call returns Option<u32> directly
                if let Some(id) = child.id() {
                    println!("Attempting fallback process termination for PID: {}", id);
                    let _ = Command::new("kill")
                        .arg("-9")
                        .arg(id.to_string())
                        .status()
                        .await;
                }
            }

            // Wait for the process to completely exit
            let _ = child.wait().await;

            // Log results of stop operation
            match stop_result {
                Ok(status) if status.success() => println!("Indexer stopped cleanly"),
                Ok(status) => println!("Indexer stop command exited with: {:?}", status),
                Err(e) => println!("Warning: Failed to run stop command: {}", e),
            }
        }

        // Verify no lingering processes
        self.cleanup_lingering_processes(project).await?;

        Ok(())
    }

    // Add new method to find and clean up any lingering processes
    async fn cleanup_lingering_processes(&self, project: &EnvioProject) -> Result<(), EnvioError> {
        // Use ps to find any lingering envio processes related to this project
        let output = Command::new("ps").arg("-ax").output().await?;

        let output_str = String::from_utf8_lossy(&output.stdout);

        // Look for processes that match the project directory
        let project_dir_str = project.dir.to_string_lossy();
        let dir_str = project_dir_str.to_string(); // Convert to String for contains check

        for line in output_str.lines() {
            if line.contains("envio") && line.contains(&dir_str) {
                // Extract PID (first column in ps output)
                if let Some(pid) = line.split_whitespace().next() {
                    if let Ok(pid) = pid.parse::<u32>() {
                        println!("Killing lingering process: {} - {}", pid, line);
                        let _ = Command::new("kill")
                            .arg("-9")
                            .arg(pid.to_string())
                            .status()
                            .await;
                    }
                }
            }
        }

        Ok(())
    }

    // New method to monitor indexer progress
    pub async fn monitor_indexer(
        &self,
        project: &EnvioProject,
    ) -> Result<IndexerStatus, EnvioError> {
        // For monitoring, we'll need to make a temporary copy since we can't modify the passed reference
        if let Some(ref process) = project.process {
            // Since we can't get a mutable reference, we'll use a different approach
            // Check if the process exists with ps command
            let output = Command::new("ps")
                .arg("-p")
                .arg(process.id().map(|id| id.to_string()).unwrap_or_default())
                .output()
                .await?;

            // If exit status is non-zero, process doesn't exist
            if !output.status.success() {
                return Ok(IndexerStatus::Stopped);
            }

            // Process exists, check GraphQL endpoint for health
            let client = reqwest::Client::new();
            match client
                .get("http://localhost:8080/health")
                .timeout(std::time::Duration::from_secs(5))
                .send()
                .await
            {
                Ok(response) if response.status().is_success() => {
                    return Ok(IndexerStatus::Running);
                }
                _ => {
                    // Still starting up
                    return Ok(IndexerStatus::Starting);
                }
            }
        }

        Ok(IndexerStatus::Stopped)
    }

    pub async fn init_project(
        &self,
        id: &str,
        contracts: Vec<ContractConfig>,
    ) -> Result<EnvioProject, EnvioError> {
        let project_dir = self.base_dir.join(id);
        std::fs::create_dir_all(&project_dir)?;

        if contracts.is_empty() {
            return Err(EnvioError::InvalidState(
                "No contracts provided for initialization".into(),
            ));
        }

        // Get ABIs and set up directory
        let abis_dir = project_dir.join("abis");
        std::fs::create_dir_all(&abis_dir)?;

        // Get ABI for each contract and write to file
        for contract in contracts.iter() {
            match self.get_abi(contract).await {
                Ok(abi) => {
                    let abi_path = abis_dir.join(format!("{}_abi.json", contract.name));
                    println!("Writing {:?}, ABI to file: {:?}", contract.name, abi_path);
                    std::fs::write(&abi_path, abi)?;
                }
                Err(_) => {
                    continue;
                }
            }
        }

        let is_first_contract_inferred = contracts[0].source.is_inferred();

        // Clone the values needed for the blocking task
        let project_dir_clone = project_dir.clone();

        std::env::set_current_dir(&project_dir_clone)?;

        let mut session = if is_first_contract_inferred {
            spawn("envio init", Some(2000))?
        } else {
            spawn("envio init contract-import local", Some(2000))?
        };
        // session.send_line("envio init contract-import local")?;

        let mut current_contract_idx = 0;
        let mut current_deployment_idx = 0;

        let mut success = false;

        loop {
            match Self::handle_envio_prompts(
                &mut session,
                &contracts,
                &mut current_contract_idx,
                &mut current_deployment_idx,
                &mut success,
            )
            .await
            {
                Ok(true) => {
                    // If we're finished, kill the process directly instead of trying to exit cleanly
                    println!("Project template ready");
                    session.send_control('c')?;
                    session.send_line("exit")?;
                    session.send_line("quit")?;
                    break;
                }
                Ok(false) => continue,
                Err(EnvioError::RexpectError(rexpect::error::Error::EOF { .. })) => break,
                Err(e) => match e {
                    EnvioError::RexpectError(rexpect::error::Error::Io(err))
                        if err.raw_os_error() == Some(5) =>
                    {
                        if success {
                            break;
                        } else {
                            return Err(EnvioError::ProcessFailed(
                                "Envio process exited unexpectedly".to_string(),
                            ));
                        }
                    }
                    _ => return Err(e),
                },
            }
        }

        println!("Waiting for envio process to exit...");
        let status = session.process.wait()?;
        match status {
            rexpect::process::wait::WaitStatus::Signaled(pid, signal, code) => {
                println!(
                    "Envio process (PID: {}) exited with signal {} code {}",
                    pid, signal, code
                );
            }
            rexpect::process::wait::WaitStatus::Exited(pid, code) => {
                println!("Envio process (PID: {}) exited with code {}", pid, code);
            }
            status => {
                println!("Envio process exited with unexpected status: {:?}", status);
                return Err(EnvioError::ProcessFailed(
                    "Envio process exited unexpectedly".to_string(),
                ));
            }
        }
        println!("Envio process completed, verifying project setup...");
        let current_dir = std::env::current_dir()?;
        println!("Current dir: {:?}", current_dir);

        // Since we're already in the project directory, just use current_dir
        let project_dir = current_dir;

        println!("Project dir: {:?}", project_dir);
        println!("Project dir exists: {:?}", project_dir.exists());

        // Use current directory for config file check
        let config_path = project_dir.join("config.yaml");
        println!("Config path: {:?}", config_path);
        println!("Config path exists: {:?}", config_path.exists());
        println!("Config path is file: {:?}", config_path.is_file());

        if !config_path.exists() {
            return Err(EnvioError::InvalidState(
                "Project initialization failed: config.yaml not created".into(),
            ));
        }

        println!("Project setup verified, returning `EnvioProject`");
        Ok(EnvioProject {
            id: id.to_string(),
            dir: project_dir,
            process: None,
        })
    }

    async fn handle_envio_prompts(
        session: &mut rexpect::session::PtySession,
        contracts: &[ContractConfig],
        current_contract_idx: &mut usize,
        current_deployment_idx: &mut usize,
        success: &mut bool,
    ) -> Result<bool, EnvioError> {
        let mut prompt = String::new();
        loop {
            match session.read_line() {
                Ok(line) => prompt.push_str(&format!("{}\n", line)),
                Err(rexpect::error::Error::EOF { .. }) => break,
                Err(_) => break,
            }
        }

        let current_prompt = prompt
            .lines()
            .rev()
            .find(|line| line.contains('?'))
            .unwrap_or("")
            .trim()
            .to_string();

        // Find options in the prompt by looking for lines with brackets or numbers
        let options: Vec<String> = prompt
            .lines()
            .filter(|line| {
                line.contains('[') || line.trim().chars().next().map_or(false, |c| c.is_numeric())
            })
            .map(|s| s.trim().to_string())
            .collect();

        if !current_prompt.is_empty() {
            println!("Current prompt: {}", current_prompt);
        }

        if !options.is_empty() {
            println!("Available options:");
            for option in options {
                println!("  {}", option);
            }
        }

        match current_prompt {
            s if s.contains("Specify a folder name") => {
                println!("Handling folder name prompt");
                session.send(".")?;
                session.flush()?;
                session.send_control('m')?;
            }
            s if s.contains("Which language would you like to use?")
                || s.contains("Javascript")
                || s.contains("Typescript")
                || s.contains("ReScript") =>
            {
                println!("Handling language selection");
                session.send_control('m')?;
            }
            s if s.contains("Choose blockchain ecosystem") => {
                println!("Handling blockchain ecosystem selection");
                // EVM and Fuel are options but for now we only support EVM
                session.send_control('m')?;
            }
            s if s.contains("Which events would you like to index?")
                || (s.contains("space to select one") && s.contains("type to filter")) =>
            {
                println!("Handling events prompt");
                session.send_control('m')?;
            }
            s if s.contains("What is the path to your json abi file?") => {
                let contract = &contracts[*current_contract_idx];
                let abi_path = format!("./abis/{}_abi.json", contract.name);

                session.send(&abi_path)?;
                session.flush()?;
                session.send_control('m')?;
            }
            s if s.contains("Would you like to import from a block explorer or a local abi") => {
                println!("Handling block explorer vs local ABI prompt");
                let contract = &contracts[*current_contract_idx];

                if contract.source.is_explorer() || contract.source.is_inferred() {
                    // For block explorer, just hit enter
                    session.send_control('m')?;
                } else {
                    // For local ABI, arrow down and hit enter
                    session.send("\x1B[B")?; // Down arrow
                    session.send_control('m')?;
                }
            }
            s if s.contains("Which blockchain would you like to import a contract from?") => {
                println!("Handling blockchain selection");
                let contract = &contracts[*current_contract_idx];
                let network_id: u64 = (&contract.deployments[*current_deployment_idx].network_id).parse().unwrap_or_default();
                // Get the network info from definitions
                let network_info = crate::network::definitions::SUPPORTED_NETWORKS
                    .get(&network_id)
                    .expect("Network ID not found in supported networks");

                // Convert network name to lowercase and convert spaces to hyphens
                let network_name = network_info.name.to_lowercase().replace(' ', "-");

                // Find index in CHAIN_LIST
                let chain_idx = crate::envio_utils::CHAIN_LIST
                    .iter()
                    .position(|&x| x == network_name)
                    .expect("Network not found in chain list");

                // Send down arrow key chain_idx times
                for _ in 0..chain_idx {
                    session.send("\x1B[B")?; // Down arrow
                }
                session.send_control('m')?;
            }
            s if s.contains("Choose network:") || s.contains("<Enter Network Id>") => {
                println!("Handling network selection");
                session.send_control('m')?;
            }
            s if s.contains("Enter the network id:") => {
                println!("Handling network id prompt");
                let contract = &contracts[*current_contract_idx];
                let deployment = &contract.deployments[*current_deployment_idx];
                session.send(&deployment.network_id.to_string())?;
                session.flush()?;
                session.send_control('m')?;
            }
            s if s.contains("What is the name of this contract?")
                || s.contains("Use the proxy address if your abi is a proxy implementation") =>
            {
                println!("Handling contract name prompt");
                let contract = &contracts[*current_contract_idx];
                session.send(&contract.name)?;
                session.flush()?;
                session.send_control('m')?;
            }
            s if s.contains("What is the address of the contract?") => {
                println!("Handling contract address prompt");
                let contract = &contracts[*current_contract_idx];
                let deployment = &contract.deployments[*current_deployment_idx];
                let address = if !deployment.address.starts_with("0x") {
                    format!("0x{}", deployment.address)
                } else {
                    deployment.address.clone()
                };
                session.send(&address)?;
                session.flush()?;
                session.send_control('m')?;
            }
            s if s.contains("Would you like to add another contract?") => {
                println!("Handling add another contract prompt");
                let contract = &contracts[*current_contract_idx];
                let deployment = &contract.deployments[*current_deployment_idx];

                // Check if there are more deployments for this contract
                if *current_deployment_idx + 1 < contract.deployments.len() {
                    let next_deployment = &contract.deployments[*current_deployment_idx + 1];
                    *current_deployment_idx += 1;

                    if next_deployment.network_id == deployment.network_id {
                        // Same network, different address
                        session.send("\x1B[B")?; // Down arrow once
                    } else {
                        // Different network
                        session.send("\x1B[B")?; // Down arrow
                        session.send("\x1B[B")?; // Down arrow again
                    }
                } else if *current_contract_idx + 1 < contracts.len() {
                    // Move to next contract
                    *current_contract_idx += 1;
                    *current_deployment_idx = 0;
                    session.send("\x1B[B")?; // Down arrow
                    session.send("\x1B[B")?; // Down arrow
                    session.send("\x1B[B")?; // Down arrow
                }
                session.flush()?;
                session.send_control('m')?;
            }
            s if s.contains("Add an API token for HyperSync to your .env file?")
                | s.contains("Add your API token:") =>
            {
                println!("Handling HyperSync API token prompt");
                session.send("\x1B[B")?;
                session.send("\x1B[B")?;
                session.flush()?;
                session.send_control('m')?;
            }
            s if s.contains("Project template ready") => {
                println!("Handling project template ready prompt");
                session.send_control('m')?;
				        return Ok(true)
            }
            s if s.contains("You can always visit 'https://envio.dev/app/api-tokens' and add a token later to your .env file.") => {
              println!("Handling final prompt");
              *success = true;
              session.send_control('m')?;
              return Ok(true)
            }
            _ => {
                if !current_prompt.is_empty() {
                    println!("Unhandled prompt: {}", current_prompt);
                    session.send_control('m')?;
                }
            }
        }

        Ok(false)
    }
    async fn get_abi(&self, contract: &ContractConfig) -> Result<String, EnvioError> {
        match &contract.source {
            ContractSource::Abi { abi, url } => match (abi, url) {
                (Some(abi_str), _) => Ok(abi_str.to_string()),
                (_, Some(url)) => fetch_abi_from_url(url).await,
                _ => Err(EnvioError::InvalidState(
                    "No ABI source provided".to_string(),
                )),
            },
            ContractSource::Explorer { api_url } => {
                let api_url = if api_url.is_empty() {
                    std::env::var("ENVIO_API_URL")
                        .unwrap_or_else(|_| "https://envio.dev/api".to_string())
                } else {
                    api_url.to_string()
                };

                fetch_abi_from_url(&api_url).await
            }
            ContractSource::Inferred => Err(EnvioError::InvalidState(
                "No ABI source provided, it is inferred from the contract address and network"
                    .to_string(),
            )),
        }
    }

    /// Subscribe to log messages from a running indexer process
    /// Returns a receiver channel that will receive log messages
    pub fn subscribe_to_logs(
        &self,
        project: &mut EnvioProject,
    ) -> Result<mpsc::Receiver<IndexerLogMessage>, EnvioError> {
        // Create a channel for sending log messages
        let (tx, rx) = mpsc::channel::<IndexerLogMessage>(100);

        if let Some(child) = &mut project.process {
            // Take ownership of stdout and stderr
            let stdout = child.stdout.take();
            let stderr = child.stderr.take();

            if let Some(stdout) = stdout {
                let tx_clone = tx.clone();

                // Use tokio's async io
                tokio::spawn(async move {
                    use tokio::io::{AsyncBufReadExt, BufReader};
                    let reader = BufReader::new(stdout);
                    let mut lines = reader.lines();

                    while let Ok(Some(line)) = lines.next_line().await {
                        let _ = tx_clone.send(IndexerLogMessage::Stdout(line.clone())).await;

                        // Try to parse progress information
                        if let Some(progress) = parse_progress_from_log(&line) {
                            let _ = tx_clone.send(IndexerLogMessage::Progress(progress)).await;
                        }
                    }
                });
            }

            if let Some(stderr) = stderr {
                let tx_clone = tx.clone();

                // Use tokio's async io
                tokio::spawn(async move {
                    use tokio::io::{AsyncBufReadExt, BufReader};
                    let reader = BufReader::new(stderr);
                    let mut lines = reader.lines();

                    while let Ok(Some(line)) = lines.next_line().await {
                        let _ = tx_clone.send(IndexerLogMessage::Stderr(line)).await;
                    }
                });
            }
        } else {
            return Err(EnvioError::InvalidState("No process running".into()));
        }

        Ok(rx)
    }
}

async fn fetch_abi_from_url(url: &str) -> Result<String, EnvioError> {
    reqwest::get(url)
        .await
        .map_err(|e| EnvioError::ProcessFailed(format!("Failed to fetch ABI: {}", e)))?
        .text()
        .await
        .map_err(|e| EnvioError::ProcessFailed(format!("Failed to read ABI response: {}", e)))
}

/// Types of log messages from an indexer
#[derive(Debug, Clone)]
pub enum IndexerLogMessage {
    /// Standard output message
    Stdout(String),
    /// Standard error message
    Stderr(String),
    /// Parsed progress information
    Progress(IndexerProgress),
}

#[derive(Debug, Clone)]
pub enum IndexerStatus {
    Configured,
    Starting,
    Running,
    Failed(String),
    Stopped,
}

#[derive(Debug, Clone, Default)]
pub struct IndexerProgress {
    pub events_processed: Option<usize>,
    pub blocks_current: Option<usize>,
    pub blocks_total: Option<usize>,
    pub chain_id: Option<String>,
    pub percentage: Option<usize>,
    pub eta: Option<String>,
}

impl From<IndexerStatus> for String {
    fn from(status: IndexerStatus) -> Self {
        match status {
            IndexerStatus::Configured => "Configured".to_string(),
            IndexerStatus::Starting => "Starting".to_string(),
            IndexerStatus::Running => "Running".to_string(),
            IndexerStatus::Failed(reason) => format!("Failed: {}", reason),
            IndexerStatus::Stopped => "Stopped".to_string(),
        }
    }
}

/// Parse progress information from a log line
fn parse_progress_from_log(line: &str) -> Option<IndexerProgress> {
    let mut progress = IndexerProgress::default();

    // Parse events processed
    if let Some(events_idx) = line.find("Events Processed:") {
        if let Some(end_idx) = line[events_idx..].find("blocks:") {
            let events_str =
                line[events_idx + "Events Processed:".len()..events_idx + end_idx].trim();
            // Remove commas and parse
            let events_str = events_str.replace(',', "");
            if let Ok(events) = events_str.parse::<usize>() {
                progress.events_processed = Some(events);
            }
        }
    }

    // Parse block information
    if let Some(blocks_idx) = line.find("blocks:") {
        let blocks_part = &line[blocks_idx + "blocks:".len()..];

        // Find current blocks
        if let Some(slash_idx) = blocks_part.find('/') {
            let current_str = blocks_part[..slash_idx].trim();
            let current_str = current_str.replace(',', "");
            if let Ok(current) = current_str.parse::<usize>() {
                progress.blocks_current = Some(current);
            }

            // Find total blocks
            let remaining = &blocks_part[slash_idx + 1..];
            if let Some(end_idx) = remaining.find(|c: char| c.is_whitespace()) {
                let total_str = &remaining[..end_idx].trim();
                let total_str = total_str.replace(',', "");
                if let Ok(total) = total_str.parse::<usize>() {
                    progress.blocks_total = Some(total);
                }
            }
        }
    }

    // Parse chain ID
    if let Some(chain_idx) = line.find("Chain ID:") {
        let chain_part = &line[chain_idx + "Chain ID:".len()..];
        if let Some(end_idx) = chain_part.find('%') {
            let chain_id = chain_part[..end_idx].trim();
            progress.chain_id = Some(chain_id.to_string());

            // Also extract percentage
            if let Some(pct) = chain_part[..end_idx]
                .trim()
                .find(|c: char| c.is_ascii_digit())
            {
                let pct_str = &chain_part[pct..end_idx].trim();
                if let Ok(percentage) = pct_str.parse::<usize>() {
                    progress.percentage = Some(percentage);
                }
            }
        }
    }

    // Parse ETA
    if let Some(eta_idx) = line.find("Sync Time ETA:") {
        let eta_part = &line[eta_idx + "Sync Time ETA:".len()..];
        if let Some(end_idx) = eta_part.find('(') {
            let eta = eta_part[..end_idx].trim();
            progress.eta = Some(eta.to_string());
        }
    }

    // Only return Some if we parsed at least one field
    if progress.events_processed.is_some()
        || progress.blocks_current.is_some()
        || progress.chain_id.is_some()
        || progress.eta.is_some()
    {
        Some(progress)
    } else {
        None
    }
}
