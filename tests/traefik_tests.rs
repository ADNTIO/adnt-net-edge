use adnt_net_edge::traefik::{RoutingConfig, render_dynamic};

fn cfg(domain: &str, path: Option<&str>, port: u16) -> RoutingConfig {
    RoutingConfig {
        domain: domain.to_string(),
        path: path.map(|p| p.to_string()),
        reverse_port: port,
    }
}

#[test]
fn renders_host_only_router() {
    let cfg = cfg("example.com", None, 1234);
    let yaml = render_dynamic(&cfg).expect("render");
    assert!(yaml.contains("Host(`example.com`)"));
    assert!(yaml.contains("url: http://127.0.0.1:1234"));
    assert!(!yaml.contains("stripPrefix"));
}

#[test]
fn renders_path_and_strip_prefix() {
    let cfg = cfg("example.com", Some("/app"), 8080);
    let yaml = render_dynamic(&cfg).expect("render");
    assert!(yaml.contains("Host(`example.com`) && PathPrefix(`/app`)"));
    assert!(yaml.contains("stripPrefix"));
    assert!(yaml.contains("/app"));
}
