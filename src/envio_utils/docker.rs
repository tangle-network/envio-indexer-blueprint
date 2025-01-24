use blueprint_sdk::std::collections::HashMap;
use blueprint_sdk::tokio::time::{sleep, Duration};
use bollard::container::{Config, CreateContainerOptions, StartContainerOptions};
use bollard::models::HostConfig;
use bollard::Docker;
use futures::StreamExt;

use crate::envio_utils::project::EnvioError;

pub struct EnvioDocker {
    client: Option<Docker>,
    postgres_id: Option<String>,
}

impl Default for EnvioDocker {
    fn default() -> Self {
        Self {
            client: None,
            postgres_id: None,
        }
    }
}

impl EnvioDocker {
    pub fn new() -> Self {
        Self {
            client: None,
            postgres_id: None,
        }
    }

    pub async fn start(&mut self) -> Result<(), EnvioError> {
        // Check if Docker daemon is running first
        if !Self::is_docker_running().await {
            return Err(EnvioError::DockerError(
                "Docker daemon is not running. Please start Docker first.".into(),
            ));
        }

        if self.client.is_none() {
            self.client = Some(Docker::connect_with_local_defaults().map_err(|e| {
                EnvioError::DockerError(format!("Failed to connect to Docker: {}", e))
            })?);
        }

        let docker = self.client.as_ref().unwrap();

        // Pull PostgreSQL image if not present
        let mut stream = docker.create_image(
            Some(bollard::image::CreateImageOptions {
                from_image: "postgres",
                tag: "13-alpine",
                ..Default::default()
            }),
            None,
            None,
        );

        while let Some(result) = stream.next().await {
            match result {
                Ok(_) => continue,
                Err(e) => return Err(EnvioError::DockerError(e.to_string())),
            }
        }

        // Create container configuration
        let mut env = HashMap::new();
        env.insert("POSTGRES_USER", "postgres");
        env.insert("POSTGRES_PASSWORD", "postgres");
        env.insert("POSTGRES_DB", "postgres");

        let env_vec: Vec<String> = env.iter().map(|(k, v)| format!("{}={}", k, v)).collect();

        let host_config = HostConfig {
            port_bindings: Some(HashMap::from([(
                "5432/tcp".to_string(),
                Some(vec![bollard::models::PortBinding {
                    host_ip: Some("0.0.0.0".to_string()),
                    host_port: Some("0".to_string()), // Let Docker assign a random port
                }]),
            )])),
            ..Default::default()
        };

        let config = Config {
            image: Some("postgres:13-alpine".to_string()),
            env: Some(env_vec),
            exposed_ports: Some(HashMap::from([("5432/tcp".to_string(), HashMap::new())])),
            host_config: Some(host_config),
            ..Default::default()
        };

        // Create and start container
        let container = docker
            .create_container(
                Some(CreateContainerOptions {
                    name: "envio-postgres",
                    platform: None,
                }),
                config,
            )
            .await
            .map_err(|e| EnvioError::DockerError(e.to_string()))?;

        docker
            .start_container(&container.id, None::<StartContainerOptions<String>>)
            .await
            .map_err(|e| EnvioError::DockerError(e.to_string()))?;

        // Get container info to find the mapped port
        let info = docker
            .inspect_container(&container.id, None)
            .await
            .map_err(|e| EnvioError::DockerError(e.to_string()))?;

        let port = info
            .network_settings
            .and_then(|s| s.ports)
            .and_then(|ports| ports.get("5432/tcp").cloned())
            .and_then(|bindings| bindings)
            .and_then(|bindings| bindings.first().cloned())
            .and_then(|binding| binding.host_port)
            .ok_or_else(|| {
                EnvioError::DockerError("Failed to get PostgreSQL container port".to_string())
            })?;

        // Wait for PostgreSQL to be ready
        sleep(Duration::from_secs(2)).await;

        // Set environment variables for Envio
        std::env::set_var("POSTGRES_HOST", "localhost");
        std::env::set_var("POSTGRES_PORT", port);
        std::env::set_var("POSTGRES_USER", "postgres");
        std::env::set_var("POSTGRES_PASSWORD", "postgres");
        std::env::set_var("POSTGRES_DB", "postgres");

        self.postgres_id = Some(container.id);
        Ok(())
    }

    pub async fn stop(&mut self) -> Result<(), EnvioError> {
        if let Some(ref client) = self.client {
            if let Some(id) = self.postgres_id.take() {
                // Stop the container
                client
                    .stop_container(&id, None)
                    .await
                    .map_err(|e| EnvioError::DockerError(e.to_string()))?;

                // Remove the container
                client
                    .remove_container(
                        &id,
                        Some(bollard::container::RemoveContainerOptions {
                            force: true,
                            ..Default::default()
                        }),
                    )
                    .await
                    .map_err(|e| EnvioError::DockerError(e.to_string()))?;
            }
        }
        Ok(())
    }

    async fn is_docker_running() -> bool {
        match Docker::connect_with_local_defaults() {
            Ok(docker) => docker.ping().await.is_ok(),
            Err(_) => false,
        }
    }
}
