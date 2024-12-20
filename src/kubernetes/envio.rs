use super::deployment::{ContainerConfig, DeploymentConfig, ResourceConfig, ServiceConfig};
use super::service::{ServiceManager, ServiceSpec, ServiceStatus};
use crate::envio_utils::{ContractSource, IndexerConfig};
use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, Serialize, Deserialize, CustomResource, JsonSchema)]
#[kube(
    group = "tangle.tools",
    version = "v1",
    kind = "EnvioIndexer",
    status = "ServiceStatus",
    derive = "Default",
    namespaced
)]
pub struct EnvioIndexerSpec {
    pub config: IndexerConfig,
}

impl ServiceSpec for EnvioIndexer {
    fn get_name(&self) -> String {
        self.spec.config.name.clone()
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

pub fn create_envio_deployment_config(
    spec: &EnvioIndexerSpec,
    namespace: &str,
) -> DeploymentConfig {
    let image_name = format!("envio-indexer-{}", spec.config.name);
    let image_tag = format!("localhost:5000/{}", image_name);

    // Create environment variables for all contracts
    let mut env = Vec::new();

    // Add environment variables for each contract
    for (idx, contract) in spec.config.contracts.iter().enumerate() {
        let prefix = if idx == 0 { "" } else { &format!("_{}", idx) };

        // Get first deployment for each contract
        if let Some(deployment) = contract.deployments.first() {
            env.extend(vec![
                (
                    format!("BLOCKCHAIN{}", prefix),
                    deployment.resolve_network_to_number(),
                ),
                (format!("RPC_URL{}", prefix), deployment.rpc_url.clone()),
                (
                    format!("CONTRACT_ADDRESS{}", prefix),
                    deployment.address.clone(),
                ),
            ]);

            if let Some(proxy) = &deployment.proxy_address {
                env.push((format!("PROXY_ADDRESS{}", prefix), proxy.clone()));
            }
        }

        // Handle API keys for explorer sources
        if let ContractSource::Explorer { api_url: api_key } = &contract.source {
            if let Some(deployment) = contract.deployments.first() {
                env.push((
                    format!(
                        "{}_VERIFIED_CONTRACT_API_TOKEN",
                        deployment.resolve_network_to_string().to_uppercase()
                    ),
                    api_key.clone(),
                ));
            }
        }
    }

    // Add the number of contracts as an environment variable
    env.push((
        "NUM_CONTRACTS".to_string(),
        spec.config.contracts.len().to_string(),
    ));

    DeploymentConfig {
        resource: ResourceConfig {
            name: spec.config.name.clone(),
            namespace: namespace.to_string(),
            labels: Default::default(),
            annotations: Default::default(),
        },
        container: ContainerConfig {
            image: image_tag,
            port: 8080,
            env,
            resources: None,
        },
        service: ServiceConfig::new(spec.config.name.clone(), namespace.to_string(), 8080),
        replicas: 1,
    }
}

pub type EnvioManagerService = ServiceManager<EnvioIndexer>;

#[cfg(test)]
mod tests {

    use crate::test_utils::{create_test_contract, create_test_explorer_contract};

    use super::*;

    #[test]
    fn test_multi_contract_indexer_spec() {
        let contracts = vec![
            create_test_contract("Contract1", "1"),
            create_test_explorer_contract("Contract2", "10"),
        ];

        let config = IndexerConfig::new("multi_test".to_string(), contracts);
        let spec = EnvioIndexerSpec { config };

        assert_eq!(spec.config.name, "multi_test");
        assert_eq!(spec.config.contracts.len(), 2);

        // Test deployment config creation
        let deployment = create_envio_deployment_config(&spec, "default");
        let env = &deployment.container.env;

        // Verify environment variables with actual addresses from test_utils
        assert!(env.iter().any(|(k, v)| k == "BLOCKCHAIN" && v == "1"));
        assert!(env.iter().any(|(k, v)| k == "BLOCKCHAIN_1" && v == "10"));
        assert!(env
            .iter()
            .any(|(k, v)| k == "CONTRACT_ADDRESS"
                && v == "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045"));
        assert!(env
            .iter()
            .any(|(k, v)| k == "CONTRACT_ADDRESS_1"
                && v == "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"));
        assert!(env
            .iter()
            .any(|(k, v)| k == "PROXY_ADDRESS_1"
                && v == "0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D"));
        assert!(env
            .iter()
            .any(|(k, v)| k == "OPTIMISM_VERIFIED_CONTRACT_API_TOKEN" && v == "test_key"));
        assert!(env.iter().any(|(k, v)| k == "NUM_CONTRACTS" && v == "2"));
    }
}
