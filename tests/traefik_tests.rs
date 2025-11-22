use adnt_net_edge::traefik::{RoutingConfig, TraefikStaticConfig, render_dynamic};

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

#[test]
fn test_routing_config_from_url_simple() {
    let cfg = RoutingConfig::from_url("https://example.com").expect("valid url");
    assert_eq!(cfg.domain, "example.com");
    assert_eq!(cfg.path, None);
}

#[test]
fn test_routing_config_from_url_with_path() {
    let cfg = RoutingConfig::from_url("https://example.com/myapp").expect("valid url");
    assert_eq!(cfg.domain, "example.com");
    assert_eq!(cfg.path, Some("/myapp".to_string()));
}

#[test]
fn test_routing_config_from_url_with_trailing_slash() {
    let cfg = RoutingConfig::from_url("https://example.com/myapp/").expect("valid url");
    assert_eq!(cfg.domain, "example.com");
    assert_eq!(cfg.path, Some("/myapp".to_string()));
}

#[test]
fn test_routing_config_from_url_root_path() {
    let cfg = RoutingConfig::from_url("https://example.com/").expect("valid url");
    assert_eq!(cfg.domain, "example.com");
    assert_eq!(cfg.path, None);
}

#[test]
fn test_routing_config_from_url_invalid() {
    let result = RoutingConfig::from_url("not-a-url");
    assert!(result.is_err());
}

#[test]
fn test_static_config_render() {
    let cfg = TraefikStaticConfig::new(
        "test@example.com",
        "/etc/traefik/dynamic.yaml",
        "/tmp/acme.json",
    );
    let rendered = cfg.render();

    assert!(rendered.contains("test@example.com"));
    assert!(rendered.contains("/etc/traefik/dynamic.yaml"));
    assert!(rendered.contains("/tmp/acme.json"));
}

#[test]
fn test_render_dynamic_with_dots_in_domain() {
    let cfg = cfg("my.sub.example.com", None, 5000);
    let yaml = render_dynamic(&cfg).expect("render");

    // Should replace dots with underscores in router/service names
    assert!(yaml.contains("my_sub_example_com"));
    assert!(yaml.contains("Host(`my.sub.example.com`)"));
}
