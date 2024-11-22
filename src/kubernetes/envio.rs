use super::deployment::{ContainerConfig, DeploymentConfig, ResourceConfig, ServiceConfig};
use super::service::{ServiceManager, ServiceSpec, ServiceStatus};
use crate::envio::EnvioError;
use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, CustomResource, JsonSchema)]
#[kube(
    group = "tangle.tools",
    version = "v1",
    kind = "EnvioIndexer",
    status = "ServiceStatus",
    derive = "Default",
    namespaced
)]
pub struct EnvioIndexerSpec {
    pub config: EnvioIndexerConfig,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EnvioIndexerConfig {
    pub name: String,
    pub abi: String,
    pub blockchain: String,
    pub rpc_url: Option<String>,
}

impl ServiceSpec for EnvioIndexer {
    fn get_name(&self) -> String {
        self.spec.config.name.clone()
    }

    fn to_deployment_config(&self, namespace: &str) -> DeploymentConfig {
        create_envio_deployment_config(&self.spec.config, namespace)
    }

    fn status(&self) -> Option<&ServiceStatus> {
        self.status.as_ref()
    }

    fn status_mut(&mut self) -> Option<&mut ServiceStatus> {
        self.status.as_mut()
    }
}

pub fn build_and_push_image(project_dir: &Path, image_name: &str) -> Result<String, EnvioError> {
    // Create Dockerfile in the project directory
    let dockerfile_content = r#"FROM ghcr.io/enviodev/envio:latest
WORKDIR /app
COPY . .
CMD ["envio", "start"]"#
        .to_string();

    std::fs::write(project_dir.join("Dockerfile"), dockerfile_content).map_err(EnvioError::Io)?;

    // Build the image
    let tag = format!("localhost:5000/{}", image_name); // Using local registry for testing
    let status = Command::new("docker")
        .args(["build", "-t", &tag, "."])
        .current_dir(project_dir)
        .status()
        .map_err(|e| EnvioError::ProcessFailed(format!("Failed to build image: {}", e)))?;

    if !status.success() {
        return Err(EnvioError::ProcessFailed("Docker build failed".into()));
    }

    // Push the image
    let status = Command::new("docker")
        .args(["push", &tag])
        .status()
        .map_err(|e| EnvioError::ProcessFailed(format!("Failed to push image: {}", e)))?;

    if !status.success() {
        return Err(EnvioError::ProcessFailed("Docker push failed".into()));
    }

    Ok(tag)
}

pub fn create_envio_deployment_config(
    spec: &EnvioIndexerConfig,
    namespace: &str,
) -> DeploymentConfig {
    let image_name = format!("envio-indexer-{}", spec.name);
    let image_tag = format!("localhost:5000/{}", image_name);

    DeploymentConfig {
        resource: ResourceConfig {
            name: spec.name.clone(),
            namespace: namespace.to_string(),
            labels: Default::default(),
            annotations: Default::default(),
        },
        container: ContainerConfig {
            image: image_tag,
            port: 8080,
            env: vec![
                ("BLOCKCHAIN".to_string(), spec.blockchain.clone()),
                (
                    "RPC_URL".to_string(),
                    spec.rpc_url.clone().unwrap_or_default(),
                ),
            ],
            resources: None,
        },
        service: ServiceConfig::new(spec.name.clone(), namespace.to_string(), 8080),
        replicas: 1,
    }
}

pub type EnvioManagerService = ServiceManager<EnvioIndexer>;
