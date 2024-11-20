use super::*;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentConfig {
    pub resource: ResourceConfig,
    pub container: ContainerConfig,
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
}

#[async_trait::async_trait]
impl ResourceManager for DeploymentManager {
    type Config = DeploymentConfig;
    type Output = Deployment;

    async fn create(&self, config: &Self::Config) -> Result<Self::Output, K8sError> {
        let api: Api<Deployment> = Api::namespaced(self.client.clone(), &config.resource.namespace);
        let pp = PostParams::default();
        let deployment = create_deployment(config);
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

fn create_deployment(config: &DeploymentConfig) -> Deployment {
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
