use deployment::{DeploymentManager, ResourceRequirements};
use k8s_openapi::api::apps::v1::Deployment;
use k8s_openapi::api::core::v1::ResourceRequirements as K8sResources;
use k8s_openapi::api::{
    apps::v1::DeploymentSpec,
    core::v1::{Container, ContainerPort, EnvVar, PodSpec, PodTemplateSpec},
};
use k8s_openapi::apimachinery::pkg::{
    api::resource::Quantity, apis::meta::v1::LabelSelector, apis::meta::v1::ObjectMeta,
};
use kube::config::InferConfigError;
use kube::Resource;
use kube::{
    api::{Api, DeleteParams, ListParams, PostParams},
    Client, Config,
};
use serde::{Deserialize, Serialize};
use service::{ServiceManager, ServiceSpec};
use thiserror::Error;

pub mod deployment;
pub mod envio;
pub mod service;

#[derive(Error, Debug)]
pub enum K8sError {
    #[error("Kube client error: {0}")]
    ClientError(#[from] kube::Error),
    #[error("Resource not found: {0}")]
    NotFound(String),
    #[error("Resource already exists: {0}")]
    AlreadyExists(String),
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
    #[error("Failed to infer Kube config: {0}")]
    KubeInferConfig(#[from] InferConfigError),
}

#[derive(Clone)]
pub struct K8sManager {
    client: Client,
    namespace: String,
}

impl K8sManager {
    pub async fn new_from_namespace(namespace: String) -> Result<Self, K8sError> {
        let config = Config::infer().await?;
        let client = Client::try_from(config)?;
        Ok(Self { client, namespace })
    }

    pub fn new(client: Client, namespace: String) -> Self {
        Self { client, namespace }
    }

    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    pub fn client(&self) -> &Client {
        &self.client
    }

    pub fn service<T: ServiceSpec>(&self) -> ServiceManager<T>
    where
        <T as Resource>::DynamicType: Default,
    {
        ServiceManager::new(self.clone())
    }

    pub fn deployments(&self) -> DeploymentManager {
        DeploymentManager::new(self.client.clone(), self.namespace.clone())
    }
}

#[async_trait::async_trait]
pub trait ResourceManager {
    type Config;
    type Output;

    async fn create(&self, config: &Self::Config) -> Result<Self::Output, K8sError>;
    async fn delete(&self, name: &str) -> Result<(), K8sError>;
    async fn get(&self, name: &str) -> Result<Self::Output, K8sError>;
    async fn list(&self) -> Result<Vec<Self::Output>, K8sError>;
}

fn metadata(
    name: &str,
    namespace: &str,
    labels: &std::collections::BTreeMap<String, String>,
    annotations: &std::collections::BTreeMap<String, String>,
) -> ObjectMeta {
    ObjectMeta {
        name: Some(name.to_string()),
        namespace: Some(namespace.to_string()),
        labels: Some(labels.clone()),
        annotations: Some(annotations.clone()),
        ..Default::default()
    }
}

fn deployment_spec(
    image: &str,
    port: u16,
    env: &[(String, String)],
    replicas: u32,
    resources: Option<ResourceRequirements>,
) -> DeploymentSpec {
    let container = Container {
        name: "app".to_string(),
        image: Some(image.to_string()),
        ports: Some(vec![ContainerPort {
            container_port: port as i32,
            ..Default::default()
        }]),
        env: Some(
            env.iter()
                .map(|(k, v)| EnvVar {
                    name: k.clone(),
                    value: Some(v.clone()),
                    ..Default::default()
                })
                .collect(),
        ),
        resources: resources.map(|r| K8sResources {
            limits: Some(
                [
                    ("cpu".to_string(), Quantity(r.cpu.clone())),
                    ("memory".to_string(), Quantity(r.memory.clone())),
                ]
                .into_iter()
                .collect(),
            ),
            requests: Some(
                [
                    ("cpu".to_string(), Quantity(r.cpu)),
                    ("memory".to_string(), Quantity(r.memory)),
                ]
                .into_iter()
                .collect(),
            ),
            ..Default::default()
        }),
        ..Default::default()
    };

    DeploymentSpec {
        replicas: Some(replicas as i32),
        selector: LabelSelector {
            match_labels: Some(
                [("app".to_string(), "envio-indexer".to_string())]
                    .into_iter()
                    .collect(),
            ),
            ..Default::default()
        },
        template: PodTemplateSpec {
            metadata: Some(ObjectMeta {
                labels: Some(
                    [("app".to_string(), "envio-indexer".to_string())]
                        .into_iter()
                        .collect(),
                ),
                ..Default::default()
            }),
            spec: Some(PodSpec {
                containers: vec![container],
                ..Default::default()
            }),
        },
        ..Default::default()
    }
}
