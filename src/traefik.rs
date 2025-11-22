// Copyright (c) 2025 ADNT Sàrl <info@adnt.io>
// License: GPL-3.0-or-later

use std::collections::BTreeMap;

use serde::Serialize;

const STATIC_TEMPLATE: &str = include_str!("../templates/traefik-static.yaml");

#[derive(Debug, Clone)]
pub struct RoutingConfig {
    pub domain: String,
    pub path: Option<String>,
    pub reverse_port: u16,
}

#[derive(Debug, Clone)]
pub struct TraefikStaticConfig {
    pub acme_email: String,
    pub dynamic_path: String,
    pub acme_storage: String,
}

impl RoutingConfig {
    pub fn from_url(url: &str) -> Result<Self, url::ParseError> {
        let parsed = url::Url::parse(url)?;
        let domain = parsed
            .host_str()
            .ok_or(url::ParseError::EmptyHost)?
            .to_string();
        let path = parsed.path().trim_end_matches('/').to_string();
        let path = if path.is_empty() || path == "/" {
            None
        } else {
            Some(path)
        };
        Ok(Self {
            domain,
            path,
            reverse_port: 0, // filled by caller
        })
    }
}

impl TraefikStaticConfig {
    pub fn new(email: impl Into<String>, dynamic_path: impl Into<String>, acme_storage: impl Into<String>) -> Self {
        Self {
            acme_email: email.into(),
            dynamic_path: dynamic_path.into(),
            acme_storage: acme_storage.into(),
        }
    }

    pub fn render(&self) -> String {
        STATIC_TEMPLATE
            .replace("{{ACME_EMAIL}}", &self.acme_email)
            .replace("{{DYNAMIC_PATH}}", &self.dynamic_path)
            .replace("{{ACME_STORAGE}}", &self.acme_storage)
    }
}

pub fn render_dynamic(config: &RoutingConfig) -> Result<String, serde_yaml::Error> {
    let router_name = format!("{}_router", config.domain.replace('.', "_"));
    let service_name = format!("{}_service", config.domain.replace('.', "_"));
    let middleware_name = format!("{}_strip", config.domain.replace('.', "_"));

    let mut routers = BTreeMap::new();
    routers.insert(
        router_name.clone(),
        Router {
            rule: build_rule(config),
            service: service_name.clone(),
            entry_points: vec!["websecure".to_string()],
            tls: Tls {
                cert_resolver: "letsencrypt".to_string(),
            },
            middlewares: build_middlewares_ref(config, &middleware_name),
        },
    );

    let mut middlewares = BTreeMap::new();
    if let Some(path) = &config.path {
        middlewares.insert(
            middleware_name,
            Middleware {
                strip_prefix: StripPrefix {
                    prefixes: vec![path.clone()],
                },
            },
        );
    }

    let mut services = BTreeMap::new();
    services.insert(
        service_name,
        Service {
            load_balancer: LoadBalancer {
                servers: vec![Server {
                    url: format!("http://127.0.0.1:{}", config.reverse_port),
                }],
            },
        },
    );

    let payload = DynamicConfig {
        http: HttpSection {
            routers,
            services,
            middlewares,
        },
    };

    serde_yaml::to_string(&payload)
}

fn build_rule(config: &RoutingConfig) -> String {
    match &config.path {
        Some(path) => format!("Host(`{}`) && PathPrefix(`{}`)", config.domain, path),
        None => format!("Host(`{}`)", config.domain),
    }
}

fn build_middlewares_ref(config: &RoutingConfig, middleware_name: &str) -> Vec<String> {
    if config.path.is_some() {
        vec![middleware_name.to_string()]
    } else {
        Vec::new()
    }
}

#[derive(Serialize)]
struct DynamicConfig {
    http: HttpSection,
}

#[derive(Serialize)]
struct HttpSection {
    routers: BTreeMap<String, Router>,
    services: BTreeMap<String, Service>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    middlewares: BTreeMap<String, Middleware>,
}

#[derive(Serialize)]
struct Router {
    rule: String,
    #[serde(rename = "service")]
    service: String,
    #[serde(rename = "entryPoints")]
    entry_points: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    middlewares: Vec<String>,
    tls: Tls,
}

#[derive(Serialize)]
struct Tls {
    #[serde(rename = "certResolver")]
    cert_resolver: String,
}

#[derive(Serialize)]
struct Service {
    #[serde(rename = "loadBalancer")]
    load_balancer: LoadBalancer,
}

#[derive(Serialize)]
struct Middleware {
    #[serde(rename = "stripPrefix")]
    strip_prefix: StripPrefix,
}

#[derive(Serialize)]
struct StripPrefix {
    prefixes: Vec<String>,
}

#[derive(Serialize)]
struct LoadBalancer {
    servers: Vec<Server>,
}

#[derive(Serialize)]
struct Server {
    url: String,
}
