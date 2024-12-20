use anyhow::Result;
use rexpect::spawn;
// use expectrl::{spawn, Regex, Session, WaitStatus};
use std::path::PathBuf;
use thiserror::Error;
use tokio::process::{Child, Command};

use super::config::{ContractConfig, ContractSource};
use super::docker::EnvioDocker;

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
    #[error("Enigo error: {0}")]
    EnigoError(#[from] enigo::InputError),
    #[error("Join error: {0}")]
    JoinError(#[from] tokio::task::JoinError),
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
    docker: EnvioDocker,
}

#[derive(Debug)]
pub struct EnvioProject {
    pub id: String,
    pub dir: PathBuf,
    pub process: Option<Child>,
}

impl EnvioManager {
    pub fn new(base_dir: PathBuf) -> Self {
        Self {
            base_dir,
            docker: EnvioDocker::new(),
        }
    }

    pub async fn start_docker(&mut self) -> Result<(), EnvioError> {
        self.docker.start().await
    }

    pub async fn stop_docker(&mut self) -> Result<(), EnvioError> {
        self.docker.stop().await
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

        let child = Command::new("envio")
            .arg("dev")
            .current_dir(&project.dir)
            .spawn()?;

        project.process = Some(child);
        Ok(())
    }

    pub async fn stop_dev(&self, project: &mut EnvioProject) -> Result<(), EnvioError> {
        if let Some(mut child) = project.process.take() {
            child.kill().await?;

            let status = Command::new("envio")
                .arg("stop")
                .current_dir(&project.dir)
                .status()
                .await?;

            if !status.success() {
                return Err(EnvioError::ProcessFailed(
                    "Failed to stop indexer cleanly".into(),
                ));
            }
        }

        Ok(())
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
            let abi = self.get_abi(contract).await?;
            let abi_path = abis_dir.join(format!("{}_abi.json", contract.name));
            println!("Writing {:?}, ABI to file: {:?}", contract.name, abi_path);
            std::fs::write(&abi_path, abi)?;
        }

        // Clone the values needed for the blocking task
        let project_dir_clone = project_dir.clone();

        std::env::set_current_dir(&project_dir_clone)?;

        let mut session = spawn("envio init contract-import local", Some(2000))?;
        // session.send_line("envio init contract-import local")?;

        let mut current_contract_idx = 0;
        let mut current_deployment_idx = 0;

        loop {
            match Self::handle_envio_prompts(
                &mut session,
                &project_dir_clone,
                &contracts,
                &mut current_contract_idx,
                &mut current_deployment_idx,
            ) {
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
                        println!("Gracefully handling I/O error");
                        break;
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

        // Verify the project directory structure
        let config_path = project_dir.join("config.yaml");
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

    fn handle_envio_prompts(
        session: &mut rexpect::session::PtySession,
        project_dir: &PathBuf,
        contracts: &[ContractConfig],
        current_contract_idx: &mut usize,
        current_deployment_idx: &mut usize,
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
            s if s.contains("Which events would you like to index?")
                || (s.contains("space to select one") && s.contains("type to filter")) =>
            {
                println!("Handling events prompt");
                session.send_control('m')?;
            }
            s if s.contains("What is the path to your json abi file?") => {
                let contract = &contracts[*current_contract_idx];
                let abi_path = project_dir
                    .join("abis")
                    .join(format!("{}_abi.json", contract.name));
                println!("ABI path: {:?}", abi_path);
                session.send(abi_path.to_str().unwrap_or_default())?;
                session.flush()?;
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
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::create_test_contract;

    #[tokio::test]
    async fn test_project_lifecycle() {
        let temp_dir = std::env::current_dir().unwrap();
        let mut manager = EnvioManager::new(temp_dir.as_path().to_path_buf());

        // Start Docker dependencies
        manager.start_docker().await.unwrap();

        // Create test contract using test utils
        let contract = create_test_contract("TestContract", "1");

        // Test project initialization
        let mut project = manager
            .init_project("test_project", vec![contract])
            .await
            .unwrap();

        // Verify project structure
        assert!(project.dir.exists());
        assert!(
            project.dir.join("config.yaml").exists(),
            "config.yaml should exist after initialization"
        );
        assert!(
            project.dir.join("abis").exists(),
            "abis directory should exist"
        );

        // Test codegen
        manager.run_codegen(&project).await.unwrap();

        // Verify generated files exist
        assert!(
            project.dir.join("src").exists(),
            "src directory should exist after codegen"
        );

        // Test dev mode
        manager.start_dev(&mut project).await.unwrap();
        assert!(project.process.is_some());

        // Test stopping
        manager.stop_dev(&mut project).await.unwrap();
        assert!(project.process.is_none());

        // Clean up
        manager.stop_docker().await.unwrap();
    }
}
