// Copyright (c) 2025 ADNT Sàrl <info@adnt.io>
// License: GPL-3.0-or-later
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// any later version.

use reqwest::header::{ACCEPT, AUTHORIZATION, USER_AGENT};
use serde::{Deserialize, Serialize};
use crate::error::{GatewayError, Result};

const GITHUB_API_BASE: &str = "https://api.github.com";

#[derive(Debug, Serialize, Deserialize)]
pub struct GitHubRepo {
    pub name: String,
    pub full_name: String,
    pub description: Option<String>,
    pub html_url: String,
    pub clone_url: String,
    pub ssh_url: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitHubRelease {
    pub tag_name: String,
    pub name: String,
    pub body: Option<String>,
    pub published_at: String,
    pub assets: Vec<GitHubAsset>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitHubAsset {
    pub name: String,
    pub browser_download_url: String,
    pub size: u64,
}

pub struct GitHubClient {
    client: reqwest::Client,
    token: String,
}

impl GitHubClient {
    pub fn new(token: String) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| GatewayError::GitHubApiError(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self { client, token })
    }

    pub async fn list_org_repos(&self, org: &str) -> Result<Vec<GitHubRepo>> {
        let url = format!("{}/orgs/{}/repos?per_page=100", GITHUB_API_BASE, org);

        let response = self
            .client
            .get(&url)
            .header(AUTHORIZATION, format!("Bearer {}", self.token))
            .header(USER_AGENT, "adnt-net-edge")
            .header(ACCEPT, "application/vnd.github+json")
            .send()
            .await
            .map_err(|e| GatewayError::GitHubApiError(format!("Request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(GatewayError::GitHubApiError(format!(
                "API request failed with status: {}",
                response.status()
            )));
        }

        let repos: Vec<GitHubRepo> = response
            .json()
            .await
            .map_err(|e| GatewayError::GitHubApiError(format!("Failed to parse response: {}", e)))?;

        Ok(repos)
    }

    pub async fn get_latest_release(&self, owner: &str, repo: &str) -> Result<GitHubRelease> {
        let url = format!(
            "{}/repos/{}/{}/releases/latest",
            GITHUB_API_BASE, owner, repo
        );

        let response = self
            .client
            .get(&url)
            .header(AUTHORIZATION, format!("Bearer {}", self.token))
            .header(USER_AGENT, "adnt-net-edge")
            .header(ACCEPT, "application/vnd.github+json")
            .send()
            .await
            .map_err(|e| GatewayError::GitHubApiError(format!("Request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(GatewayError::GitHubApiError(format!(
                "Failed to get latest release: {}",
                response.status()
            )));
        }

        let release: GitHubRelease = response
            .json()
            .await
            .map_err(|e| GatewayError::GitHubApiError(format!("Failed to parse response: {}", e)))?;

        Ok(release)
    }

    pub async fn list_tools(&self, org: &str) -> Result<Vec<GitHubRepo>> {
        let repos = self.list_org_repos(org).await?;

        // Filter repos that are ADNT tools (you can customize this filter)
        let tools: Vec<GitHubRepo> = repos
            .into_iter()
            .filter(|repo| {
                // Filter for tools - customize based on naming convention
                repo.name.starts_with("adnt-") ||
                repo.description.as_ref().map_or(false, |d| d.contains("tool"))
            })
            .collect();

        Ok(tools)
    }

    #[allow(dead_code)]
    pub async fn download_asset(&self, url: &str, path: &std::path::Path) -> Result<()> {
        let response = self
            .client
            .get(url)
            .header(AUTHORIZATION, format!("Bearer {}", self.token))
            .header(USER_AGENT, "adnt-net-edge")
            .header(ACCEPT, "application/octet-stream")
            .send()
            .await
            .map_err(|e| GatewayError::GitHubApiError(format!("Download failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(GatewayError::GitHubApiError(format!(
                "Download failed with status: {}",
                response.status()
            )));
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|e| GatewayError::GitHubApiError(format!("Failed to read bytes: {}", e)))?;

        std::fs::write(path, bytes)
            .map_err(|e| GatewayError::WriteFile(e))?;

        Ok(())
    }
}
