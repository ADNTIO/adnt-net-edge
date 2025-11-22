// Copyright (c) 2025 ADNT Sàrl <info@adnt.io>
// License: GPL-3.0-or-later
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// any later version.

use std::io::ErrorKind;
use std::process::Stdio;

use tokio::process::{Child, Command};

use crate::error::{GatewayError, Result};

#[derive(Debug, Clone)]
pub struct TunnelConfig {
    pub host: String,
    pub user: String,
    pub ssh_port: u16,
    pub reverse_port: u16,
    pub local_port: u16,
    pub keep_alive_secs: u64,
    pub extra_args: Vec<String>,
    pub identity_file: Option<String>,
}

pub struct SshTunnel {
    child: Child,
}

impl SshTunnel {
    // Skip coverage: requires actual SSH binary and external system
    pub async fn start(config: TunnelConfig) -> Result<Self> {
        let args = build_args(&config);
        tracing::info!("starting ssh tunnel: ssh {}", args.join(" "));

        let mut command = Command::new("ssh");
        command
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .kill_on_drop(true);

        let child = command.spawn().map_err(|err| match err.kind() {
            ErrorKind::NotFound => GatewayError::SshMissing,
            _ => GatewayError::SshSpawn(err),
        })?;

        Ok(Self { child })
    }

    // Skip coverage: requires actual SSH process
    pub async fn wait(&mut self) -> Result<()> {
        let status = self.child.wait().await?;
        if status.success() {
            tracing::info!("ssh tunnel exited cleanly");
            Ok(())
        } else {
            tracing::error!("ssh tunnel exited with status {status}");
            Err(GatewayError::SshExit(status))
        }
    }
}

fn build_args(config: &TunnelConfig) -> Vec<String> {
    let mut args = vec![
        "-NT".to_string(),
        "-p".to_string(),
        config.ssh_port.to_string(),
        "-o".to_string(),
        format!("ServerAliveInterval={}", config.keep_alive_secs),
        "-o".to_string(),
        "ExitOnForwardFailure=yes".to_string(),
        "-o".to_string(),
        "ServerAliveCountMax=3".to_string(),
        "-R".to_string(),
        format!("{}:127.0.0.1:{}", config.reverse_port, config.local_port),
    ];

    if let Some(identity) = &config.identity_file {
        args.push("-i".to_string());
        args.push(identity.clone());
    }

    args.extend(config.extra_args.clone());
    args.push(format!("{}@{}", config.user, config.host));

    args
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_args_basic() {
        let config = TunnelConfig {
            host: "example.com".to_string(),
            user: "testuser".to_string(),
            ssh_port: 22,
            reverse_port: 8080,
            local_port: 3000,
            keep_alive_secs: 30,
            extra_args: Vec::new(),
            identity_file: None,
        };

        let args = build_args(&config);

        assert_eq!(args[0], "-NT");
        assert_eq!(args[1], "-p");
        assert_eq!(args[2], "22");
        assert!(args.contains(&"ServerAliveInterval=30".to_string()));
        assert!(args.contains(&"8080:127.0.0.1:3000".to_string()));
        assert_eq!(args.last().unwrap(), "testuser@example.com");
    }

    #[test]
    fn test_build_args_with_identity() {
        let config = TunnelConfig {
            host: "example.com".to_string(),
            user: "root".to_string(),
            ssh_port: 2222,
            reverse_port: 9000,
            local_port: 4000,
            keep_alive_secs: 60,
            extra_args: Vec::new(),
            identity_file: Some("/path/to/key".to_string()),
        };

        let args = build_args(&config);

        assert!(args.contains(&"-i".to_string()));
        assert!(args.contains(&"/path/to/key".to_string()));
    }

    #[test]
    fn test_build_args_with_extra_args() {
        let config = TunnelConfig {
            host: "example.com".to_string(),
            user: "admin".to_string(),
            ssh_port: 22,
            reverse_port: 8080,
            local_port: 3000,
            keep_alive_secs: 30,
            extra_args: vec![
                "-v".to_string(),
                "-o".to_string(),
                "ConnectTimeout=10".to_string(),
            ],
            identity_file: None,
        };

        let args = build_args(&config);

        assert!(args.contains(&"-v".to_string()));
        assert!(args.contains(&"ConnectTimeout=10".to_string()));
    }
}
