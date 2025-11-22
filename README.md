# adnt-net-edge

Lightweight SSH + Traefik edge tunnel (similar to Ngrok) running on a small VPS to provide your local HTTP service with HTTPS using Let’s Encrypt.

## Features
- SSH reverse tunnel with keepalive options.
- Remote port auto-allocation by default (or pick a fixed port).
- Traefik dynamic config generation (Host/PathPrefix with optional strip).
- One-shot remote Traefik deploy in Docker (host network) with ACME TLS, using temp files under `/tmp/adnt-net-edge`.
- Graceful shutdown: Ctrl+C stops Traefik (container) and deletes the temp configs on the remote host.

## Requirements
- Rust toolchain (for building).
- Remote host reachable by SSH (default user `root`, port `22`):
  - Able to start Docker containers.
  - Able to bind on 80/443 (Traefik host network).
  - Has `ssh` and `scp` available.

## Usage
Build and run:
```bash
cargo run -- --url https://prod.example.com/app --port 8080 \
  --traefik-acme-email you@example.com
```

Key flags:
- `--url <public-url>`: **[REQUIRED]** public URL (host + optional path) to route.
- `--port <local-port>`: **[REQUIRED]** local port to expose through the tunnel.
- `--traefik-acme-email <email>`: **[REQUIRED]** ACME email for Let's Encrypt certificates.
- `--deploy-traefik`: deploy Traefik on the remote server (default: **enabled**). Use `--deploy-traefik=false` to disable.
- `--ssh-user` / `--ssh-host` / `--ssh-port`: SSH connection params (default: user `root`, host from `--url`, port `22`).
- `--remote-port`: reverse port on the remote (default `0` → auto on server).
- `--traefik-static-path` / `--traefik-dynamic-path`: override remote config paths (defaults: `/tmp/adnt-net-edge/traefik.yaml` and `/tmp/adnt-net-edge/dynamic.yaml`).
- `--identity`: SSH key file if not using the default agent.

### How it works
1) Picks/allocates a remote port (default auto) and generates Traefik dynamic config targeting `http://127.0.0.1:<remote_port>` with Host/PathPrefix rules derived from `--url`.
2) Deploys Traefik (optional) via SSH/SCP + `docker run --network host --configFile=/tmp/adnt-net-edge/traefik.yaml`.
3) Starts an SSH reverse tunnel `-R <remote_port>:127.0.0.1:<local_port>`.
4) On Ctrl+C, stops Traefik and removes `/tmp/adnt-net-edge/*` on the remote.

### Quick verification
- Local: `curl http://localhost:<port>` should return your app.
- Remote (after tunnel + Traefik): `curl https://<public-host>/<path>` should match the local response (stripPrefix applied when a path is present in `--url`).

## Development
Run unit tests:
```bash
cargo test
```

CI: GitHub Actions runs fmt, tests, coverage (tarpaulin >= 90%) on Linux, and fmt/tests + ssh/scp availability on Windows.

## License
Copyright (c) 2025 ADNT Sàrl <info@adnt.io>

Licensed under the GNU General Public License v3.0 or later. See `LICENSE`.
