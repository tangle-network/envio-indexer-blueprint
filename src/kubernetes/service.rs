use std::fmt::Debug;

use super::*;
use deployment::DeploymentConfig;
use gadget_sdk::futures::TryStreamExt;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::Time;
use k8s_openapi::chrono;
use kube::api::{Patch, PatchParams};
use kube::core::NamespaceResourceScope;
use kube::runtime::watcher::{watcher, Config, Event};
use kube::Resource;
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use tokio::time::{sleep, Duration};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TimeWrapper(pub Time);

impl JsonSchema for TimeWrapper {
    fn schema_name() -> String {
        "Time".to_string()
    }

    fn json_schema(gen: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        <String>::json_schema(gen)
    }
}

mod time_serde {
    use super::*;
    use serde::{Deserializer, Serializer};

    pub fn serialize<S>(time: &TimeWrapper, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        time.0 .0.to_rfc3339().serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<TimeWrapper, D::Error>
    where
        D: Deserializer<'de>,
    {
        let time_str = String::deserialize(deserializer)?;
        let dt =
            chrono::DateTime::parse_from_rfc3339(&time_str).map_err(serde::de::Error::custom)?;
        Ok(TimeWrapper(Time(dt.with_timezone(&chrono::Utc))))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct ServiceStatus {
    pub phase: ServicePhase,
    pub message: Option<String>,
    pub last_updated: Option<TimeWrapper>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
pub enum ServicePhase {
    Pending,
    Starting,
    Running,
    Failed,
    Terminated,
}

pub trait ServiceSpec:
    Clone
    + Send
    + Sync
    + Resource<Scope = NamespaceResourceScope>
    + DeserializeOwned
    + JsonSchema
    + Debug
    + Serialize
    + 'static
{
    fn get_name(&self) -> String;
    fn to_deployment_config(&self, namespace: &str) -> DeploymentConfig;
    fn status(&self) -> Option<&ServiceStatus>;
    fn status_mut(&mut self) -> Option<&mut ServiceStatus>;
}
pub struct ServiceManager<T: ServiceSpec> {
    k8s: K8sManager,
    _phantom: std::marker::PhantomData<T>,
}

impl<T: ServiceSpec> ServiceManager<T>
where
    <T as kube::Resource>::DynamicType: std::default::Default,
{
    pub fn new(k8s: K8sManager) -> Self {
        Self {
            k8s,
            _phantom: std::marker::PhantomData,
        }
    }

    pub async fn create(&self, spec: &T) -> Result<T, K8sError> {
        let api: Api<T> = Api::namespaced(self.k8s.client.clone(), &self.k8s.namespace);
        let pp = PostParams::default();

        // Create the custom resource
        let res = api.create(&pp, spec).await?;

        // Create the associated deployment
        let config = spec.to_deployment_config(&self.k8s.namespace);
        self.k8s.deployments().create(&config).await?;

        Ok(res)
    }

    pub async fn delete(&self, name: &str) -> Result<(), K8sError> {
        let api: Api<T> = Api::namespaced(self.k8s.client.clone(), &self.k8s.namespace);
        let dp = DeleteParams::default();

        // Delete the custom resource
        api.delete(name, &dp).await?;

        // Delete the associated deployment
        self.k8s.deployments().delete(name).await?;

        Ok(())
    }

    pub async fn get(&self, name: &str) -> Result<T, K8sError> {
        let api: Api<T> = Api::namespaced(self.k8s.client.clone(), &self.k8s.namespace);
        api.get(name).await.map_err(|e| match e {
            kube::Error::Api(err) if err.code == 404 => K8sError::NotFound(name.to_string()),
            e => K8sError::ClientError(e),
        })
    }

    pub async fn list(&self) -> Result<Vec<T>, K8sError> {
        let api: Api<T> = Api::namespaced(self.k8s.client.clone(), &self.k8s.namespace);
        let lp = ListParams::default();
        let res = api.list(&lp).await?;
        Ok(res.items)
    }

    pub async fn update_status(
        &self,
        name: &str,
        phase: ServicePhase,
        message: Option<String>,
    ) -> Result<(), K8sError> {
        let api: Api<T> = Api::namespaced(self.k8s.client.clone(), &self.k8s.namespace);
        let status = ServiceStatus {
            phase,
            message,
            last_updated: Some(TimeWrapper(Time(chrono::Utc::now()))),
        };

        api.patch_status(
            name,
            &PatchParams::default(),
            &Patch::Merge(&serde_json::json!({ "status": status })),
        )
        .await?;

        Ok(())
    }

    pub async fn watch_and_reconcile(&self) {
        loop {
            let api: Api<T> = Api::namespaced(self.k8s.client.clone(), &self.k8s.namespace);
            let watcher = watcher(api, Config::default());

            if let Err(e) = watcher
                .try_for_each(|event| async {
                    match event {
                        Event::Apply(svc) => {
                            if let Err(e) = self.reconcile_service(&svc).await {
                                tracing::error!("Failed to reconcile service: {}", e);
                            }
                        }
                        Event::Delete(_) => {}
                        _ => {}
                    }
                    Ok(())
                })
                .await
            {
                eprintln!("Watch error: {}", e);
                sleep(Duration::from_secs(5)).await;
            }
        }
    }

    async fn reconcile_service(&self, svc: &T) -> Result<(), K8sError> {
        let name = svc.get_name();
        let config = svc.to_deployment_config(&self.k8s.namespace);

        match self.k8s.deployments().get(&name).await {
            Ok(deployment) => {
                // Update status based on deployment state
                let available_replicas = deployment
                    .status
                    .and_then(|s| s.available_replicas)
                    .unwrap_or(0);

                let phase = if available_replicas > 0 {
                    ServicePhase::Running
                } else {
                    ServicePhase::Starting
                };

                self.update_status(&name, phase, None).await?;
            }
            Err(K8sError::NotFound(_)) => {
                // Create deployment if it doesn't exist
                self.k8s.deployments().create(&config).await?;
                self.update_status(&name, ServicePhase::Starting, None)
                    .await?;
            }
            Err(e) => {
                self.update_status(&name, ServicePhase::Failed, Some(e.to_string()))
                    .await?;
                return Err(e);
            }
        }

        Ok(())
    }
}
