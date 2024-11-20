use super::deployment::{ContainerConfig, DeploymentConfig, ResourceConfig};
use super::service::{ServiceManager, ServiceSpec, ServiceStatus};
use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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
    pub spec: EnvioIndexerConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<ServiceStatus>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EnvioIndexerConfig {
    pub name: String,
    pub abi: String,
    pub blockchain: String,
    pub rpc_url: Option<String>,
}

impl ServiceSpec for EnvioIndexerSpec {
    fn get_name(&self) -> String {
        self.spec.name.clone()
    }

    fn to_deployment_config(&self, namespace: &str) -> DeploymentConfig {
        create_envio_deployment_config(&self.spec, namespace)
    }

    fn status(&self) -> Option<&ServiceStatus> {
        self.status.as_ref()
    }

    fn status_mut(&mut self) -> Option<&mut ServiceStatus> {
        self.status.as_mut()
    }
}

fn create_envio_deployment_config(spec: &EnvioIndexerConfig, namespace: &str) -> DeploymentConfig {
    DeploymentConfig {
        resource: ResourceConfig {
            name: spec.name.clone(),
            namespace: namespace.to_string(),
            labels: [
                ("app".to_string(), "envio-indexer".to_string()),
                ("indexer".to_string(), spec.name.clone()),
            ]
            .into_iter()
            .collect(),
            annotations: Default::default(),
        },
        container: ContainerConfig {
            image: "envio-indexer:latest".to_string(), // This should be configurable
            port: 8000,                                // This should be configurable
            env: vec![
                ("BLOCKCHAIN".to_string(), spec.blockchain.clone()),
                ("ABI".to_string(), spec.abi.clone()),
            ],
            resources: None, // Add resource limits if needed
        },
        replicas: 1,
    }
}

pub type EnvioManager = ServiceManager<EnvioIndexerSpec>;
