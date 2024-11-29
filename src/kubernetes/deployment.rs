use super::*;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig {
    pub name: String,
    pub port: u16,
    pub target_port: u16,
    pub namespace: String,
}

impl ServiceConfig {
    pub fn new(name: String, namespace: String, external_port: u16) -> Self {
        Self {
            name,
            port: external_port,
            target_port: 8080,
            namespace,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentConfig {
    pub resource: ResourceConfig,
    pub container: ContainerConfig,
    pub service: ServiceConfig,
    pub replicas: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceConfig {
    pub name: String,
    pub namespace: String,
    pub labels: BTreeMap<String, String>,
    pub annotations: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerConfig {
    pub image: String,
    pub port: u16,
    pub env: Vec<(String, String)>,
    pub resources: Option<ResourceRequirements>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceRequirements {
    pub cpu: String,
    pub memory: String,
}

pub struct DeploymentManager {
    client: Client,
    namespace: String,
}

impl DeploymentManager {
    pub fn new(client: Client, namespace: String) -> Self {
        Self { client, namespace }
    }

    pub async fn create(&self, config: &DeploymentConfig) -> Result<(), K8sError> {
        let deployment = self.build_deployment(config);
        let deployments: Api<Deployment> =
            Api::namespaced(self.client.clone(), &config.resource.namespace);

        match deployments
            .create(&PostParams::default(), &deployment)
            .await
        {
            Ok(_) => (),
            Err(kube::Error::Api(err)) if err.code == 409 => {
                return Err(K8sError::AlreadyExists(config.resource.name.clone()))
            }
            Err(e) => return Err(K8sError::ClientError(e)),
        }

        let service = self.build_service(config)?;
        let services: Api<k8s_openapi::api::core::v1::Service> =
            Api::namespaced(self.client.clone(), &config.resource.namespace);

        match services.create(&PostParams::default(), &service).await {
            Ok(_) => Ok(()),
            Err(kube::Error::Api(err)) if err.code == 409 => {
                Err(K8sError::AlreadyExists(config.resource.name.clone()))
            }
            Err(e) => Err(K8sError::ClientError(e)),
        }
    }

    fn build_deployment(&self, config: &DeploymentConfig) -> Deployment {
        Deployment {
            metadata: metadata(
                &config.resource.name,
                &config.resource.namespace,
                &config.resource.labels,
                &config.resource.annotations,
            ),
            spec: Some(deployment_spec(
                &config.container.image,
                config.container.port,
                &config.container.env,
                config.replicas,
                config.container.resources.clone(),
            )),
            ..Default::default()
        }
    }

    fn build_service(
        &self,
        config: &DeploymentConfig,
    ) -> Result<k8s_openapi::api::core::v1::Service, K8sError> {
        let mut labels = config.resource.labels.clone();
        labels.insert("app".to_string(), config.resource.name.clone());

        Ok(k8s_openapi::api::core::v1::Service {
            metadata: ObjectMeta {
                name: Some(config.resource.name.clone()),
                namespace: Some(config.resource.namespace.clone()),
                labels: Some(labels.clone()),
                ..Default::default()
            },
            spec: Some(k8s_openapi::api::core::v1::ServiceSpec {
                ports: Some(vec![k8s_openapi::api::core::v1::ServicePort {
                    port: config.service.port as i32,
                    target_port: Some(
                        k8s_openapi::apimachinery::pkg::util::intstr::IntOrString::Int(
                            config.container.port as i32,
                        ),
                    ),
                    ..Default::default()
                }]),
                selector: Some(labels),
                type_: Some("ClusterIP".to_string()),
                ..Default::default()
            }),
            status: None,
        })
    }
}

#[async_trait::async_trait]
impl ResourceManager for DeploymentManager {
    type Config = DeploymentConfig;
    type Output = Deployment;

    async fn create(&self, config: &Self::Config) -> Result<Self::Output, K8sError> {
        let api: Api<Deployment> = Api::namespaced(self.client.clone(), &config.resource.namespace);
        let pp = PostParams::default();
        let deployment = self.build_deployment(config);
        let res = api.create(&pp, &deployment).await?;
        Ok(res)
    }

    async fn delete(&self, name: &str) -> Result<(), K8sError> {
        let api: Api<Deployment> = Api::namespaced(self.client.clone(), &self.namespace);
        let dp = DeleteParams::default();
        api.delete(name, &dp).await?;
        Ok(())
    }

    async fn get(&self, name: &str) -> Result<Self::Output, K8sError> {
        let api: Api<Deployment> = Api::namespaced(self.client.clone(), &self.namespace);
        api.get(name).await.map_err(|e| match e {
            kube::Error::Api(err) if err.code == 404 => K8sError::NotFound(name.to_string()),
            e => K8sError::ClientError(e),
        })
    }

    async fn list(&self) -> Result<Vec<Self::Output>, K8sError> {
        let api: Api<Deployment> = Api::namespaced(self.client.clone(), &self.namespace);
        let lp = ListParams::default();
        let res = api.list(&lp).await?;
        Ok(res.items)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kube::{Client, Config};
    use rustls::crypto::aws_lc_rs::default_provider;

    async fn setup_test_client() -> (Client, String) {
        let provider = default_provider();
        let _ = provider.install_default();
        let config = Config::infer().await.expect("Failed to infer kube config");
        let client = Client::try_from(config).expect("Failed to create kube client");
        (client, "test-namespace".to_string())
    }

    fn create_test_config() -> DeploymentConfig {
        DeploymentConfig {
            resource: ResourceConfig {
                name: "test-indexer".to_string(),
                namespace: "test-namespace".to_string(),
                labels: Default::default(),
                annotations: Default::default(),
            },
            container: ContainerConfig {
                image: "localhost:5000/test-image:latest".to_string(),
                port: 8080,
                env: vec![
                    ("BLOCKCHAIN".to_string(), "ethereum".to_string()),
                    ("RPC_URL".to_string(), "http://localhost:8545".to_string()),
                ],
                resources: None,
            },
            service: ServiceConfig::new(
                "test-indexer".to_string(),
                "test-namespace".to_string(),
                8080,
            ),
            replicas: 1,
        }
    }

    #[tokio::test]
    async fn test_build_deployment() {
        let (client, namespace) = setup_test_client().await;
        let manager = DeploymentManager::new(client, namespace);
        let config = create_test_config();

        let deployment = manager.build_deployment(&config);

        // Verify deployment metadata
        assert_eq!(deployment.metadata.name, Some("test-indexer".to_string()));
        assert_eq!(
            deployment.metadata.namespace,
            Some("test-namespace".to_string())
        );

        // Verify deployment spec
        let spec = deployment.spec.unwrap();
        assert_eq!(spec.replicas, Some(1));

        let container = &spec.template.spec.unwrap().containers[0];
        assert_eq!(
            container.image,
            Some("localhost:5000/test-image:latest".to_string())
        );
        assert_eq!(container.ports.as_ref().unwrap()[0].container_port, 8080);
    }

    #[tokio::test]
    async fn test_build_service() {
        let (client, namespace) = setup_test_client().await;
        let manager = DeploymentManager::new(client, namespace);
        let config = create_test_config();

        let service = manager
            .build_service(&config)
            .expect("Failed to build service");

        assert_eq!(service.metadata.name, Some("test-indexer".to_string()));
        assert_eq!(
            service.metadata.namespace,
            Some("test-namespace".to_string())
        );

        let spec = service.spec.unwrap();
        assert_eq!(spec.type_, Some("ClusterIP".to_string()));

        let port = &spec.ports.unwrap()[0];
        assert_eq!(port.port, 8080);
        assert_eq!(
            port.target_port,
            Some(k8s_openapi::apimachinery::pkg::util::intstr::IntOrString::Int(8080))
        );
    }
}
