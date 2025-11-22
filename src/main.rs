// Copyright (c) 2025 ADNT Sàrl <info@adnt.io>
// License: GPL-3.0-or-later
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// any later version.


mod error;
mod ssh;
mod traefik;

use std::path::PathBuf;

use clap::Parser;
use tempfile::NamedTempFile;
use tracing_subscriber::EnvFilter;
use url::Url;

use crate::error::{GatewayError, Result};
use crate::ssh::TunnelConfig;
use crate::traefik::{RoutingConfig, TraefikStaticConfig};

#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "Expose a port local au dev via un tunnel SSH et un proxy Traefik"
)]
struct Cli {
    /// Public URL to expose (e.g. https://public-gateway.example/test)
    #[arg(long, value_name = "URL")]
    url: String,

    /// Local port to expose
    #[arg(long, value_name = "PORT")]
    port: u16,

    /// SSH user (default: root)
    #[arg(long, default_value = "root")]
    ssh_user: String,

    /// SSH host (default: host extracted from --url)
    #[arg(long)]
    ssh_host: Option<String>,

    /// SSH port (default: 22)
    #[arg(long, default_value_t = 22)]
    ssh_port: u16,

    /// Remote port to reserve for the reverse tunnel (0 = auto)
    #[arg(long, default_value_t = 0, value_name = "PORT")]
    remote_port: u16,

    /// Deploy Traefik on the remote server (Docker)
    #[arg(long, default_value_t = false)]
    deploy_traefik: bool,

    /// ACME email for Let's Encrypt (required if --deploy-traefik)
    #[arg(long)]
    traefik_acme_email: Option<String>,

    /// Remote path for Traefik static config
    #[arg(long, default_value = "/tmp/adnt-net-edge/traefik.yaml")]
    traefik_static_path: String,

    /// Remote path for Traefik dynamic config
    #[arg(long, default_value = "/tmp/adnt-net-edge/dynamic.yaml")]
    traefik_dynamic_path: String,

    /// SSH private key to use
    #[arg(long)]
    identity: Option<PathBuf>,

    /// Output file for generated Traefik dynamic config (stdout if omitted)
    #[arg(long)]
    traefik_output: Option<PathBuf>,

    /// Log filter (e.g. info, debug)
    #[arg(long, default_value = "info")]
    log: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    init_tracing(&cli.log);

    let mut cfg = build_config(&cli)?;
    cfg.reverse_port = resolve_remote_port(&cfg, cli.remote_port).await?;
    tracing::info!("port distant retenu: {}", cfg.reverse_port);
    let mut routing = RoutingConfig::from_url(&cli.url)?;
    routing.reverse_port = cfg.reverse_port;
    let dynamic_yaml = render_traefik(&routing, cli.traefik_output.as_ref())?;

    let deployment = if cli.deploy_traefik {
        Some(deploy_traefik(&cfg, &dynamic_yaml, &cli).await?)
    } else {
        None
    };

    run_with_shutdown(cfg, deployment).await?;
    Ok(())
}

#[cfg_attr(tarpaulin, skip)]
fn build_config(cli: &Cli) -> Result<TunnelConfig> {
    let parsed_remote = Url::parse(&cli.url).map_err(GatewayError::InvalidUrl)?;
    let ssh_host = cli
        .ssh_host
        .clone()
        .or_else(|| parsed_remote.host_str().map(|h| h.to_string()))
        .ok_or_else(|| GatewayError::InvalidUrl(url::ParseError::EmptyHost))?;

    Ok(TunnelConfig {
        host: ssh_host,
        user: cli.ssh_user.clone(),
        ssh_port: cli.ssh_port,
        reverse_port: cli.remote_port,
        local_port: cli.port,
        keep_alive_secs: 30,
        extra_args: Vec::new(),
        identity_file: cli
            .identity
            .as_ref()
            .map(|p| p.to_string_lossy().to_string()),
    })
}

#[cfg_attr(tarpaulin, skip)]
async fn run_tunnel(tunnel_cfg: TunnelConfig) -> Result<()> {
    let mut tunnel = ssh::SshTunnel::start(tunnel_cfg).await?;
    tunnel.wait().await
}

fn render_traefik(routing: &RoutingConfig, output: Option<&PathBuf>) -> Result<String> {
    let yaml = traefik::render_dynamic(routing).map_err(GatewayError::TraefikRender)?;
    match output {
        Some(path) => {
            std::fs::write(path, &yaml).map_err(GatewayError::WriteFile)?;
            tracing::info!("Config Traefik générée dans {}", path.display());
        }
        None => println!("{yaml}"),
    }
    Ok(yaml)
}

fn init_tracing(filter: &str) {
    let env_filter = EnvFilter::try_new(filter).unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(env_filter).init();
}

async fn run_with_shutdown(tunnel_cfg: TunnelConfig, deployment: Option<RemoteDeployment>) -> Result<()> {
    let mut tunnel = ssh::SshTunnel::start(tunnel_cfg.clone()).await?;

    let ctrl_c = tokio::signal::ctrl_c();
    tokio::pin!(ctrl_c);

    let result = tokio::select! {
        _ = &mut ctrl_c => {
            tracing::info!("received Ctrl+C, shutting down");
            Ok(())
        }
        res = tunnel.wait() => res,
    };

    if let Some(dep) = deployment {
        dep.cleanup(&tunnel_cfg).await.ok();
    }

    result
}

#[cfg_attr(tarpaulin, skip)]
async fn resolve_remote_port(tunnel: &TunnelConfig, requested: u16) -> Result<u16> {
    if requested != 0 {
        return Ok(requested);
    }
    pick_remote_free_port(tunnel).await
}

#[cfg_attr(tarpaulin, skip)]
async fn pick_remote_free_port(tunnel: &TunnelConfig) -> Result<u16> {
    let script = "python3 - <<'PY'\nimport socket\ns=socket.socket()\ns.bind(('',0))\nprint(s.getsockname()[1])\nPY";
    let output = run_remote_capture(tunnel, script).await?;
    let port: u16 = output.trim().parse()?;
    Ok(port)
}

#[cfg_attr(tarpaulin, skip)]
async fn deploy_traefik(tunnel: &TunnelConfig, dynamic_yaml: &str, cli: &Cli) -> Result<RemoteDeployment> {
    let email = cli
        .traefik_acme_email
        .as_ref()
        .ok_or(GatewayError::MissingAcmeEmail)?;

    let static_cfg = TraefikStaticConfig::new(
        email,
        &cli.traefik_dynamic_path,
        format!("{}/acme.json", remote_base_dir()),
    );

    let static_temp = NamedTempFile::new().map_err(GatewayError::WriteFile)?;
    std::fs::write(static_temp.path(), static_cfg.render()).map_err(GatewayError::WriteFile)?;

    let dynamic_temp = NamedTempFile::new().map_err(GatewayError::WriteFile)?;
    std::fs::write(dynamic_temp.path(), dynamic_yaml).map_err(GatewayError::WriteFile)?;

    // Ensure directories exist before uploads.
    let prepare_dirs = format!(
        "mkdir -p $(dirname {static_path}) $(dirname {dynamic_path}) /etc/traefik",
        static_path = cli.traefik_static_path,
        dynamic_path = cli.traefik_dynamic_path
    );
    run_remote_command(tunnel, &prepare_dirs).await?;

    scp_upload(
        tunnel,
        static_temp.path(),
        &format!("{}:{}", remote_target(tunnel), cli.traefik_static_path),
    )
    .await?;

    scp_upload(
        tunnel,
        dynamic_temp.path(),
        &format!("{}:{}", remote_target(tunnel), cli.traefik_dynamic_path),
    )
    .await?;

    let base_dir = remote_base_dir();
    let acme_path = format!("{base_dir}/acme.json");
    let remote_cmd = format!(
        "set -euo pipefail; \
         mkdir -p {base_dir}; \
         touch {acme}; chmod 600 {acme}; \
         (systemctl stop apache2 || true); \
         (systemctl stop nginx || true); \
         docker rm -f traefik || true; \
         docker run -d --name traefik --restart=unless-stopped \
           --network host \
           -v {static_path}:{static_path}:ro \
           -v {dynamic_path}:{dynamic_path}:ro \
           -v {acme}:{acme} \
           traefik:v3.0 --configFile={static_path}",
        base_dir = base_dir,
        acme = acme_path,
        static_path = cli.traefik_static_path,
        dynamic_path = cli.traefik_dynamic_path
    );

    run_remote_command(tunnel, &remote_cmd).await?;
    tracing::info!("Traefik déployé et démarré sur {}", tunnel.host);
    Ok(RemoteDeployment {
        static_path: cli.traefik_static_path.clone(),
        dynamic_path: cli.traefik_dynamic_path.clone(),
        acme_path,
        container_name: "traefik".to_string(),
    })
}

#[cfg_attr(tarpaulin, skip)]
async fn scp_upload(tunnel: &TunnelConfig, local: &std::path::Path, remote: &str) -> Result<()> {
    let mut args = Vec::new();
    args.push("-P".to_string());
    args.push(tunnel.ssh_port.to_string());
    args.push("-o".to_string());
    args.push("StrictHostKeyChecking=no".to_string());
    args.push("-o".to_string());
    args.push("UserKnownHostsFile=/dev/null".to_string());
    if let Some(id) = &tunnel.identity_file {
        args.push("-i".to_string());
        args.push(id.clone());
    }
    args.push(local.to_string_lossy().to_string());
    args.push(remote.to_string());

    let mut command = tokio::process::Command::new("scp");
    command
        .args(&args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit());

    tracing::info!("upload via scp {}", args.join(" "));
    let mut status = command.spawn().map_err(|err| match err.kind() {
        std::io::ErrorKind::NotFound => GatewayError::ScpMissing,
        _ => GatewayError::ScpSpawn(err),
    })?;
    let status = status.wait().await?;
    if !status.success() {
        return Err(GatewayError::ScpExit(status));
    }
    Ok(())
}

#[cfg_attr(tarpaulin, skip)]
async fn run_remote_command(tunnel: &TunnelConfig, cmd: &str) -> Result<()> {
    let status = run_remote_status(tunnel, cmd).await?;
    if !status.success() {
        return Err(GatewayError::SshExit(status));
    }
    Ok(())
}

#[cfg_attr(tarpaulin, skip)]
async fn run_remote_status(tunnel: &TunnelConfig, cmd: &str) -> Result<std::process::ExitStatus> {
    let mut args = Vec::new();
    args.push("-p".to_string());
    args.push(tunnel.ssh_port.to_string());
    args.push("-o".to_string());
    args.push("StrictHostKeyChecking=no".to_string());
    args.push("-o".to_string());
    args.push("UserKnownHostsFile=/dev/null".to_string());
    if let Some(id) = &tunnel.identity_file {
        args.push("-i".to_string());
        args.push(id.clone());
    }

    args.push(remote_target(tunnel));
    args.push(cmd.to_string());

    let mut command = tokio::process::Command::new("ssh");
    command
        .args(&args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .kill_on_drop(true);

    tracing::info!("ssh {}", args.join(" "));
    let mut status = command.spawn().map_err(|err| match err.kind() {
        std::io::ErrorKind::NotFound => GatewayError::SshMissing,
        _ => GatewayError::SshSpawn(err),
    })?;
    let status = status.wait().await?;
    Ok(status)
}

#[cfg_attr(tarpaulin, skip)]
async fn run_remote_capture(tunnel: &TunnelConfig, cmd: &str) -> Result<String> {
    let mut args = Vec::new();
    args.push("-p".to_string());
    args.push(tunnel.ssh_port.to_string());
    args.push("-o".to_string());
    args.push("StrictHostKeyChecking=no".to_string());
    args.push("-o".to_string());
    args.push("UserKnownHostsFile=/dev/null".to_string());
    if let Some(id) = &tunnel.identity_file {
        args.push("-i".to_string());
        args.push(id.clone());
    }

    args.push(remote_target(tunnel));
    args.push(cmd.to_string());

    let mut command = tokio::process::Command::new("ssh");
    command
        .args(&args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::inherit())
        .kill_on_drop(true);

    tracing::info!("ssh {}", args.join(" "));
    let child = command.spawn().map_err(|err| match err.kind() {
        std::io::ErrorKind::NotFound => GatewayError::SshMissing,
        _ => GatewayError::SshSpawn(err),
    })?;

    let output = child.wait_with_output().await?;
    if !output.status.success() {
        return Err(GatewayError::SshExit(output.status));
    }

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    Ok(stdout)
}

fn remote_target(tunnel: &TunnelConfig) -> String {
    format!("{}@{}", tunnel.user, tunnel.host)
}

fn remote_base_dir() -> String {
    "/tmp/adnt-net-edge".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_config_defaults_host_from_url() {
        let cli = Cli {
            url: "https://example.com/app".into(),
            port: 8080,
            ssh_user: "root".into(),
            ssh_host: None,
            ssh_port: 22,
            remote_port: 0,
            deploy_traefik: false,
            traefik_acme_email: None,
            traefik_static_path: "/etc/traefik/traefik.yaml".into(),
            traefik_dynamic_path: "/etc/traefik/dynamic.yaml".into(),
            identity: None,
            traefik_output: None,
            log: "info".into(),
        };

        let cfg = build_config(&cli).expect("config");
        assert_eq!(cfg.host, "example.com");
        assert_eq!(cfg.user, "root");
        assert_eq!(cfg.local_port, 8080);
    }
}
struct RemoteDeployment {
    static_path: String,
    dynamic_path: String,
    acme_path: String,
    container_name: String,
}

impl RemoteDeployment {
    async fn cleanup(&self, tunnel: &TunnelConfig) -> Result<()> {
        let cmd = format!(
            "set -euo pipefail; \
             docker rm -f {container} || true; \
             rm -f {static_path} {dynamic_path} {acme_path} || true; \
             rmdir {base} 2>/dev/null || true",
            container = self.container_name,
            static_path = self.static_path,
            dynamic_path = self.dynamic_path,
            acme_path = self.acme_path,
            base = remote_base_dir()
        );
        run_remote_command(tunnel, &cmd).await
    }
}
