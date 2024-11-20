use std::path::PathBuf;
use std::process::Stdio;
use thiserror::Error;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};

#[derive(Error, Debug)]
pub enum EnvioError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Failed to capture process output: {0}")]
    ProcessOutput(String),
    #[error("Process failed: {0}")]
    ProcessFailed(String),
    #[error("Invalid project state: {0}")]
    InvalidState(String),
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

    pub async fn init_project(&self, id: &str) -> Result<EnvioProject, EnvioError> {
        let project_dir = self.base_dir.join(id);
        std::fs::create_dir_all(&project_dir)?;

        let mut child = Command::new("envio")
            .arg("init")
            .arg(id)
            .current_dir(&self.base_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;

        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| EnvioError::ProcessOutput("Failed to open stdin".into()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| EnvioError::ProcessOutput("Failed to capture stdout".into()))?;

        let mut reader = BufReader::new(stdout).lines();

        // Handle interactive prompts
        while let Ok(Some(line)) = reader.next_line().await {
            match line.trim() {
                l if l.contains("Create a new project?") => {
                    stdin.write_all(b"y\n").await?;
                }
                l if l.contains("Enter project name") => {
                    stdin.write_all(format!("{}\n", id).as_bytes()).await?;
                }
                l if l.contains("Use default configuration?") => {
                    stdin.write_all(b"y\n").await?;
                }
                _ => continue,
            }
        }

        drop(stdin);

        let status = child.wait().await?;

        if !status.success() {
            return Err(EnvioError::ProcessFailed(
                "Failed to initialize envio project".into(),
            ));
        }

        Ok(EnvioProject {
            id: id.to_string(),
            dir: project_dir,
            process: None,
        })
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

    pub async fn check_health(&self, project: &mut EnvioProject) -> Result<bool, EnvioError> {
        if let Some(child) = &mut project.process {
            match child.try_wait()? {
                Some(status) => Ok(status.success()),
                None => Ok(true), // Process is still running
            }
        } else {
            Ok(false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_project_lifecycle() {
        let temp_dir = tempdir().unwrap();
        let manager = EnvioManager::new(temp_dir.path().to_path_buf());

        // Test project initialization
        let mut project = manager.init_project("test_project").await.unwrap();
        assert!(project.dir.exists());

        // Test codegen
        manager.run_codegen(&project).await.unwrap();

        // Test dev mode
        manager.start_dev(&mut project).await.unwrap();
        assert!(project.process.is_some());

        // Test health check
        assert!(manager.check_health(&mut project).await.unwrap());

        // Test stopping
        manager.stop_dev(&mut project).await.unwrap();
        assert!(project.process.is_none());
    }
}
