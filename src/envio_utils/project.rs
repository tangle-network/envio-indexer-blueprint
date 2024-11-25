use anyhow::{anyhow, Context, Result};
use enigo::{Direction, Enigo, Key, Keyboard};

use envio::{
    clap_definitions::{InitArgs, ProjectPaths},
    config_parsing::{
        chain_helpers::{HypersyncNetwork, Network},
        contract_import::converters::{
            ContractImportNetworkSelection, NetworkKind, SelectedContract,
        },
        entity_parsing::Schema,
        system_config::SystemConfig,
    },
    constants::project_paths::{self, DEFAULT_CONFIG_PATH, DEFAULT_GENERATED_PATH},
    executor::init::run_init_args,
    init_config::{
        self,
        evm::{ContractImportSelection, InitFlow},
        InitConfig,
    },
};
use expectrl::{check, spawn, Regex, Session, WaitStatus};
use fake::faker::address::en;
use std::{
    io::{BufRead, Read, Write},
    path::PathBuf,
    time::Duration,
};
use thiserror::Error;
use tokio::io::AsyncBufReadExt;
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
            let abi = self.get_abi(&contract).await?;
            let abi_path = abis_dir.join(format!("{}_abi.json", contract.name));
            std::fs::write(&abi_path, abi)?;
        }

        // Clone the values needed for the blocking task
        let project_dir_clone = project_dir.clone();
        let contracts_clone = contracts.to_vec();
        let id_clone = id.to_string();

        tokio::task::spawn_blocking(move || {
            std::env::set_current_dir(&project_dir_clone).map_err(|e| {
                EnvioError::ProcessFailed(format!("Failed to set directory: {}", e))
            })?;

            let mut session = spawn("envio init contract-import local")
                .map_err(|e| EnvioError::ProcessFailed(format!("Failed to spawn envio: {}", e)))?;

            let mut current_contract_idx = 0;
            let mut current_deployment_idx = 0;

            loop {
                Self::handle_envio_prompt(
                    &mut session,
                    &contracts,
                    &mut current_contract_idx,
                    &mut current_deployment_idx,
                )?;
            }

            let status = session.get_process().wait().map_err(|e| {
                EnvioError::ProcessFailed(format!("Failed to wait for process: {}", e))
            })?;

            match status {
                WaitStatus::Exited(_, code) if code == 0 => Ok(()),
                _ => Err(EnvioError::ProcessFailed("Init process failed".into())),
            }
        })
        .await??;

        Ok(EnvioProject {
            id: id.to_string(),
            dir: project_dir,
            process: None,
        })
    }

    fn handle_envio_prompt(
        session: &mut Session,
        contracts: &[ContractConfig],
        current_contract_idx: &mut usize,
        current_deployment_idx: &mut usize,
    ) -> Result<(), EnvioError> {
        // session.set_expect_lazy(true);
        let captures = session.expect(Regex(r"\?.*"))?;
        let prompt = String::from_utf8_lossy(&captures.as_bytes()).to_string();
        println!("Processing prompt: {}", prompt);

        match prompt.trim() {
            s if s.contains("Specify a folder name") => {
                println!("Handling folder name prompt");
                session.send_line("\n")?;
                session.expect(Regex(r".*\n"))?;
                session.flush()?;
                std::thread::sleep(std::time::Duration::from_millis(500));
            }
            s if s.contains("Which language would you like to use?") => {
                session.send_line("TypeScript\n")?;
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            s if s.contains("Which events would you like to index?") => {
                session.send_line("\n")?;
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            s if s.contains("What is the address of the contract?") => {
                let contract = &contracts[*current_contract_idx];
                let deployment = &contract.deployments[*current_deployment_idx];

                let address = if !deployment.address.starts_with("0x") {
                    format!("0x{}", deployment.address)
                } else {
                    deployment.address.clone()
                };

                let response = format!("{}\n", address);
                session.send_line(&response)?;
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            s if s.contains("Would you like to add another contract?") => {
                let contract = &contracts[*current_contract_idx];
                let response = if *current_deployment_idx + 1 < contract.deployments.len() {
                    *current_deployment_idx += 1;
                    String::from("y\n")
                } else if *current_contract_idx + 1 < contracts.len() {
                    *current_contract_idx += 1;
                    *current_deployment_idx = 0;
                    String::from("y\n")
                } else {
                    String::from("n\n")
                };
                session.send_line(&response)?;
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            s if s.contains("Choose network:") => {
                session.send_line("\n")?;
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            s if s.contains("Enter the network id:") => {
                let contract = &contracts[*current_contract_idx];
                let deployment = &contract.deployments[*current_deployment_idx];
                let network_id = deployment.resolve_network_to_number();
                let response = format!("{}\n", network_id);
                session.send_line(&response)?;
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            s if s.contains("Please provide an rpc url") => {
                let contract = &contracts[*current_contract_idx];
                let deployment = &contract.deployments[*current_deployment_idx];
                let response = format!("{}\n", deployment.rpc_url);
                session.send_line(&response)?;
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            s if s.contains("Please provide a start block") => {
                let contract = &contracts[*current_contract_idx];
                let deployment = &contract.deployments[*current_deployment_idx];
                let start_block = deployment.start_block.unwrap_or(0);
                let response = format!("{}\n", start_block);
                session.send_line(&response)?;
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            s if s.contains("Would you like to import from a block explorer or a local abi?") => {
                let response = match &contracts[*current_contract_idx].source {
                    ContractSource::Explorer { .. } => String::from("1\n"),
                    ContractSource::Abi { .. } => String::from("2\n"),
                };
                session.send_line(&response)?;
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            s if s.contains("Which blockchain would you like to import a contract from?") => {
                let contract = &contracts[*current_contract_idx];
                let deployment = &contract.deployments[*current_deployment_idx];
                let response = format!("{}\n", deployment.resolve_network_to_string());
                session.send_line(&response)?;
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            s if s.contains("What is the path to your json abi file?") => {
                let contract = &contracts[*current_contract_idx];
                let response = format!("./abis/{}_abi.json\n", contract.name);
                session.send_line(&response)?;
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            s if s.contains("What is the name of this contract?") => {
                let contract = &contracts[*current_contract_idx];
                let response = format!("{}\n", contract.name);
                session.send_line(&response)?;
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            _ => {
                session.send_line("\n")?;
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        };

        Ok(())
    }

    // pub async fn init_project(
    //     &self,
    //     id: &str,
    //     contracts: &[ContractConfig],
    // ) -> Result<EnvioProject, EnvioError> {
    //     let project_dir = self.base_dir.join(id);
    //     std::fs::create_dir_all(&project_dir)?;

    //     let selected_contracts = self.envio_selected_contracts(contracts);
    //     let project_paths = ProjectPaths {
    //         directory: project_dir.to_str().map(String::from),
    //         output_directory: String::from(DEFAULT_GENERATED_PATH),
    //         config: String::from(DEFAULT_CONFIG_PATH),
    //     };

    //     self.init(selected_contracts, &project_paths);

    //     Ok(EnvioProject {
    //         id: id.to_string(),
    //         dir: project_dir,
    //         process: None,
    //     })
    // }

    // pub async fn envio_selected_contracts(
    //     &self,
    //     contracts: &[ContractConfig],
    // ) -> Result<Vec<SelectedContract>, EnvioError> {
    //     let mut selected_contracts = Vec::new();

    //     for contract in contracts {
    //         let abi_str = self.get_abi(contract).await?;
    //         let abi: ethers::abi::Contract = serde_json::from_str(&abi_str)
    //             .map_err(|e| EnvioError::ProcessFailed(format!("Failed to parse ABI: {}", e)))?;
    //         let mut all_events = Vec::new();
    //         let mut events_iter = abi.events();
    //         while let Some(event) = events_iter.next() {
    //             all_events.push(event.clone());
    //         }

    //         let networks = contract
    //             .deployments
    //             .iter()
    //             .map(|deployment| {
    //                 let network_id = deployment.network_id.parse().unwrap();
    //                 let network = match Network::from_network_id(network_id) {
    //                     Ok(network) => match HypersyncNetwork::try_from(network) {
    //                         Ok(hypersync_network) => NetworkKind::Supported(hypersync_network),
    //                         Err(_) => NetworkKind::Unsupported {
    //                             network_id,
    //                             rpc_url: deployment.rpc_url.clone(),
    //                             start_block: deployment.start_block.unwrap_or(0),
    //                         },
    //                     },
    //                     Err(_) => NetworkKind::Unsupported {
    //                         network_id,
    //                         rpc_url: deployment.rpc_url.clone(),
    //                         start_block: deployment.start_block.unwrap_or(0),
    //                     },
    //                 };
    //                 ContractImportNetworkSelection {
    //                     network,
    //                     addresses: vec![deployment.address.parse().unwrap()],
    //                 }
    //             })
    //             .collect();

    //         let selected_contract = SelectedContract {
    //             name: contract.name.clone(),
    //             networks,
    //             events: all_events,
    //         };

    //         selected_contracts.push(selected_contract);
    //     }

    //     selected_contracts
    // }

    // pub async fn init(
    //     &self,
    //     name: String,
    //     selected_contracts: Vec<SelectedContract>,
    //     project_paths: &ProjectPaths,
    // ) -> Result<(), EnvioError> {
    //     let selected_contract_config = ContractImportSelection { selected_contracts };
    //     let init_config = InitConfig {
    //         name,
    //         directory: project_paths.directory.unwrap_or_default(),
    //         ecosystem: init_config::Ecosystem::Evm {
    //             init_flow: InitFlow::ContractImport(selected_contract_config),
    //         },
    //         language: init_config::Language::TypeScript,
    //         api_token: None,
    //     };

    //     let parsed_project_paths = ParsedProjectPaths::try_from(init_config.clone())
    //         .context("Failed parsing paths from interactive input")?;

    //     let evm_config = selected_contract_config
    //         .to_human_config(&init_config)
    //         .context("Failed to converting auto config selection into config.yaml")?;

    //     // TODO: Allow parsed paths to not depend on a written config.yaml file in file system
    //     tokio::fs::write(project_paths.join("config.yaml"), evm_config.to_string())
    //         .await
    //         .context("failed writing imported config.yaml")?;

    //     //Use an empty schema config to generate auto_schema_handler_template
    //     //After it's been generated, the schema exists and codegen can parse it/use it
    //     let system_config =
    //         SystemConfig::from_evm_config(evm_config, Schema::empty(), &parsed_project_paths)
    //             .context("Failed parsing config")?;

    //     let auto_schema_handler_template =
    //         contract_import_templates::AutoSchemaHandlerTemplate::try_from(
    //             system_config,
    //             &init_config.language,
    //             init_config.api_token.clone(),
    //         )
    //         .context("Failed converting config to auto auto_schema_handler_template")?;

    //     template_dirs
    //         .get_and_extract_blank_template(
    //             &init_config.language,
    //             &parsed_project_paths.project_root,
    //         )
    //         .context(format!(
    //             "Failed initializing blank template for Contract Import with language {} at \
    //        path {:?}",
    //             &init_config.language, &parsed_project_paths.project_root,
    //         ))?;

    //     auto_schema_handler_template
    //         .generate_contract_import_templates(
    //             &init_config.language,
    //             &parsed_project_paths.project_root,
    //         )
    //         .context(
    //             "Failed generating contract import templates for schema and event handlers.",
    //         )?;

    //     Ok(())
    // }

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
