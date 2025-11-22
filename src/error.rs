// Copyright (c) 2025 ADNT Sàrl <info@adnt.io>
// License: GPL-3.0-or-later
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// any later version.

use std::process::ExitStatus;

#[derive(thiserror::Error, Debug)]
pub enum GatewayError {
    #[error("invalid url: {0}")]
    InvalidUrl(#[from] url::ParseError),
    #[error("failed to render traefik config: {0}")]
    TraefikRender(serde_yaml::Error),
    #[error("missing ACME email for Traefik deployment")]
    MissingAcmeEmail,
    #[error("failed to write file: {0}")]
    WriteFile(std::io::Error),
    #[error("failed to parse port: {0}")]
    PortParse(#[from] std::num::ParseIntError),
    #[error("scp binary not found in PATH")]
    ScpMissing,
    #[error("failed to spawn scp: {0}")]
    ScpSpawn(std::io::Error),
    #[error("scp exited with status {0}")]
    ScpExit(ExitStatus),
    #[error("ssh binary not found in PATH")]
    SshMissing,
    #[error("failed to spawn ssh: {0}")]
    SshSpawn(#[from] std::io::Error),
    #[error("ssh exited with status {0}")]
    SshExit(ExitStatus),
}

pub type Result<T> = std::result::Result<T, GatewayError>;
