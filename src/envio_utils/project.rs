use anyhow::Result;
use rexpect::spawn_bash;
// use expectrl::{spawn, Regex, Session, WaitStatus};
use std::{io::Write, path::PathBuf};
use thiserror::Error;
use tokio::process::{Child, Command};

use super::config::{ContractConfig, ContractSource};

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
    #[error("Expect error: {0}")]
    ExpectError(#[from] expectrl::Error),
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
        let status = Command::new("envio")
            .arg("codegen")
            .current_dir(&project.dir)
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
            std::fs::write(&abi_path, abi)?;
        }

        // Clone the values needed for the blocking task
        let project_dir_clone = project_dir.clone();
        let contracts_clone = contracts.to_vec();
        let id_clone = id.to_string();

        tokio::task::spawn_blocking(move || {
            std::env::set_current_dir(&project_dir_clone)?;

            let mut session = spawn_bash(Some(2000))?;
            session.send_line("envio init contract-import local")?;

            let mut current_contract_idx = 0;
            let mut current_deployment_idx = 0;

            loop {
                match Self::handle_envio_prompts(
                    &mut session,
                    &contracts,
                    &mut current_contract_idx,
                    &mut current_deployment_idx,
                ) {
                    Ok(()) => continue,
                    Err(EnvioError::RexpectError(rexpect::error::Error::EOF { .. })) => break,
                    Err(e) => return Err(e),
                }
            }

            let status = session.process.wait()?;
            match status {
                rexpect::process::wait::WaitStatus::Exited(_, _) => (),
                _ => {
                    return Err(EnvioError::ProcessFailed(
                        "Envio process exited unexpectedly".to_string(),
                    ))
                }
            }

            Ok(())
        })
        .await??;

        Ok(EnvioProject {
            id: id.to_string(),
            dir: project_dir,
            process: None,
        })
    }

    fn handle_envio_prompts(
        session: &mut rexpect::session::PtySession,
        contracts: &[ContractConfig],
        current_contract_idx: &mut usize,
        current_deployment_idx: &mut usize,
    ) -> Result<(), EnvioError> {
        let (_, prompt) = session.exp_regex(r"\?.*")?;
        println!("Processing prompt: {}", prompt);

        match prompt.trim() {
            s if s.contains("Specify a folder name") => {
                println!("Handling folder name prompt");
                session.send_line("")?;
                session.exp_regex(r"\n")?;
            }
            s if s.contains("Which language would you like to use?") => {
                println!("Handling language prompt");
                session.send_line("TypeScript")?;
                session.exp_regex(r"\n")?;
            }
            s if s.contains("Which events would you like to index?") => {
                println!("Handling events prompt");
                session.send_line("")?;
                session.exp_regex(r"\n")?;
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
                session.send_line(&address)?;
                session.exp_regex(r"\n")?;
            }
            s if s.contains("Would you like to add another contract?") => {
                println!("Handling add contract prompt");
                let contract = &contracts[*current_contract_idx];
                let response = if *current_deployment_idx + 1 < contract.deployments.len() {
                    *current_deployment_idx += 1;
                    "y"
                } else if *current_contract_idx + 1 < contracts.len() {
                    *current_contract_idx += 1;
                    *current_deployment_idx = 0;
                    "y"
                } else {
                    "n"
                };
                session.send_line(response)?;
                session.exp_regex(r"\n")?;
            }
            s if s.contains("Choose network:") => {
                println!("Handling network choice prompt");
                session.send_line("")?;
                session.exp_regex(r"\n")?;
            }
            s if s.contains("Enter the network id:") => {
                println!("Handling network ID prompt");
                let contract = &contracts[*current_contract_idx];
                let deployment = &contract.deployments[*current_deployment_idx];
                let network_id = deployment.resolve_network_to_number();
                session.send_line(&network_id.to_string())?;
                session.exp_regex(r"\n")?;
            }
            s if s.contains("Please provide an rpc url") => {
                println!("Handling RPC URL prompt");
                let contract = &contracts[*current_contract_idx];
                let deployment = &contract.deployments[*current_deployment_idx];
                session.send_line(&deployment.rpc_url)?;
                session.exp_regex(r"\n")?;
            }
            s if s.contains("Please provide a start block") => {
                println!("Handling start block prompt");
                let contract = &contracts[*current_contract_idx];
                let deployment = &contract.deployments[*current_deployment_idx];
                let start_block = deployment.start_block.unwrap_or(0);
                session.send_line(&start_block.to_string())?;
                session.exp_regex(r"\n")?;
            }
            s if s.contains("Would you like to import from a block explorer or a local abi?") => {
                println!("Handling import source prompt");
                match &contracts[*current_contract_idx].source {
                    ContractSource::Explorer { .. } => session.send_line("1")?,
                    ContractSource::Abi { .. } => session.send_line("2")?,
                };
                session.exp_regex(r"\n")?;
            }
            s if s.contains("Which blockchain would you like to import a contract from?") => {
                println!("Handling blockchain selection prompt");
                let contract = &contracts[*current_contract_idx];
                let deployment = &contract.deployments[*current_deployment_idx];
                session.send_line(&deployment.resolve_network_to_string())?;
                session.exp_regex(r"\n")?;
            }
            s if s.contains("What is the path to your json abi file?") => {
                println!("Handling ABI path prompt");
                let contract = &contracts[*current_contract_idx];
                session.send_line(&format!("./abis/{}_abi.json", contract.name))?;
                session.exp_regex(r"\n")?;
            }
            s if s.contains("What is the name of this contract?") => {
                println!("Handling contract name prompt");
                let contract = &contracts[*current_contract_idx];
                session.send_line(&contract.name)?;
                session.exp_regex(r"\n")?;
            }
            _ => {
                println!("Unhandled prompt: {}", prompt);
                session.send_line("")?;
                session.exp_regex(r"\n")?;
            }
        }

        Ok(())
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
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_project_lifecycle() {
        let temp_dir = tempdir().unwrap();
        let manager = EnvioManager::new(temp_dir.path().to_path_buf());

        // Create test contract using test utils
        let contract = create_test_contract("TestContract", "1");

        // Test project initialization
        let mut project = manager
            .init_project("test_project", vec![contract])
            .await
            .unwrap();
        assert!(project.dir.exists());

        // Test codegen
        manager.run_codegen(&project).await.unwrap();

        // Test dev mode
        manager.start_dev(&mut project).await.unwrap();
        assert!(project.process.is_some());

        // Test stopping
        manager.stop_dev(&mut project).await.unwrap();
        assert!(project.process.is_none());
    }
}
