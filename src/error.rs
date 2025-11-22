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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display_invalid_url() {
        let err = GatewayError::InvalidUrl(url::ParseError::EmptyHost);
        assert!(err.to_string().contains("invalid url"));
    }

    #[test]
    fn test_error_display_ssh_missing() {
        let err = GatewayError::SshMissing;
        assert_eq!(err.to_string(), "ssh binary not found in PATH");
    }

    #[test]
    fn test_error_display_scp_missing() {
        let err = GatewayError::ScpMissing;
        assert_eq!(err.to_string(), "scp binary not found in PATH");
    }

    #[test]
    fn test_error_display_missing_acme_email() {
        let err = GatewayError::MissingAcmeEmail;
        assert_eq!(err.to_string(), "missing ACME email for Traefik deployment");
    }

    #[test]
    fn test_error_from_parse_int() {
        let parse_err = "not_a_number".parse::<u16>().unwrap_err();
        let gateway_err: GatewayError = parse_err.into();
        assert!(gateway_err.to_string().contains("failed to parse port"));
    }
}
