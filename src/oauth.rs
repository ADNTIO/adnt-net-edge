// Copyright (c) 2025 ADNT Sàrl <info@adnt.io>
// License: GPL-3.0-or-later
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// any later version.

use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, PkceCodeChallenge,
    RedirectUrl, Scope, TokenResponse, TokenUrl,
};
use oauth2::basic::BasicClient;
use oauth2::reqwest::async_http_client;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use crate::error::{GatewayError, Result};

const GITHUB_AUTH_URL: &str = "https://github.com/login/oauth/authorize";
const GITHUB_TOKEN_URL: &str = "https://github.com/login/oauth/access_token";

pub struct GitHubOAuthConfig {
    pub client_id: String,
    pub client_secret: String,
}

impl Default for GitHubOAuthConfig {
    fn default() -> Self {
        Self {
            client_id: "Ov23lihUc287puYq0CK1".to_string(),
            client_secret: std::env::var("ADNT_GITHUB_CLIENT_SECRET")
                .unwrap_or_else(|_| String::new()),
        }
    }
}

pub struct OAuthClient {
    client: BasicClient,
}

impl OAuthClient {
    pub fn new(config: GitHubOAuthConfig) -> Result<Self> {
        let client = BasicClient::new(
            ClientId::new(config.client_id),
            Some(ClientSecret::new(config.client_secret)),
            AuthUrl::new(GITHUB_AUTH_URL.to_string())
                .map_err(|e| GatewayError::OAuthError(format!("Invalid auth URL: {}", e)))?,
            Some(
                TokenUrl::new(GITHUB_TOKEN_URL.to_string())
                    .map_err(|e| GatewayError::OAuthError(format!("Invalid token URL: {}", e)))?,
            ),
        )
        .set_redirect_uri(
            RedirectUrl::new("http://localhost:8080/callback".to_string())
                .map_err(|e| GatewayError::OAuthError(format!("Invalid redirect URL: {}", e)))?,
        );

        Ok(Self { client })
    }

    pub async fn authenticate(&self) -> Result<String> {
        let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

        let (auth_url, csrf_token) = self
            .client
            .authorize_url(CsrfToken::new_random)
            .add_scope(Scope::new("repo".to_string()))
            .add_scope(Scope::new("user".to_string()))
            .set_pkce_challenge(pkce_challenge)
            .url();

        println!("\n🔐 GitHub OAuth Authentication");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("\nPlease open this URL in your browser:");
        println!("\n{}\n", auth_url);
        println!("Waiting for authentication...\n");

        let (code, state) = Self::listen_for_callback()?;

        if state.secret() != csrf_token.secret() {
            return Err(GatewayError::OAuthError(
                "CSRF token mismatch".to_string(),
            ));
        }

        let token_result = self
            .client
            .exchange_code(AuthorizationCode::new(code))
            .set_pkce_verifier(pkce_verifier)
            .request_async(async_http_client)
            .await
            .map_err(|e| GatewayError::OAuthError(format!("Token exchange failed: {}", e)))?;

        let access_token = token_result.access_token().secret().to_string();

        println!("✅ Authentication successful!\n");

        Ok(access_token)
    }

    fn listen_for_callback() -> Result<(String, CsrfToken)> {
        let listener = TcpListener::bind("127.0.0.1:8080")
            .map_err(|e| GatewayError::OAuthError(format!("Failed to bind to port 8080: {}", e)))?;

        if let Ok((mut stream, _)) = listener.accept() {
            let mut reader = BufReader::new(&stream);
            let mut request_line = String::new();
            reader
                .read_line(&mut request_line)
                .map_err(|e| GatewayError::OAuthError(format!("Failed to read request: {}", e)))?;

            let redirect_url = request_line
                .split_whitespace()
                .nth(1)
                .ok_or_else(|| GatewayError::OAuthError("Invalid request".to_string()))?;

            let url = url::Url::parse(&format!("http://localhost{}", redirect_url))
                .map_err(|e| GatewayError::OAuthError(format!("Failed to parse callback URL: {}", e)))?;

            let code = url
                .query_pairs()
                .find(|(key, _)| key == "code")
                .map(|(_, value)| value.to_string())
                .ok_or_else(|| GatewayError::OAuthError("No authorization code".to_string()))?;

            let state = url
                .query_pairs()
                .find(|(key, _)| key == "state")
                .map(|(_, value)| CsrfToken::new(value.to_string()))
                .ok_or_else(|| GatewayError::OAuthError("No state parameter".to_string()))?;

            let response = "HTTP/1.1 200 OK\r\n\
                           Content-Type: text/html; charset=utf-8\r\n\r\n\
                           <html><body>\
                           <h1>✅ Authentication Successful!</h1>\
                           <p>You can close this window and return to the terminal.</p>\
                           </body></html>";

            stream
                .write_all(response.as_bytes())
                .map_err(|e| GatewayError::OAuthError(format!("Failed to write response: {}", e)))?;

            Ok((code, state))
        } else {
            Err(GatewayError::OAuthError(
                "Failed to accept connection".to_string(),
            ))
        }
    }
}

pub fn store_token(token: &str) -> Result<()> {
    let entry = keyring::Entry::new("adnt-net-edge", "github_token")
        .map_err(|e| GatewayError::OAuthError(format!("Failed to create keyring entry: {}", e)))?;

    entry
        .set_password(token)
        .map_err(|e| GatewayError::OAuthError(format!("Failed to store token: {}", e)))?;

    tracing::info!("GitHub token stored securely in system keyring");
    Ok(())
}

pub fn get_stored_token() -> Result<Option<String>> {
    let entry = keyring::Entry::new("adnt-net-edge", "github_token")
        .map_err(|e| GatewayError::OAuthError(format!("Failed to create keyring entry: {}", e)))?;

    match entry.get_password() {
        Ok(token) => Ok(Some(token)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(GatewayError::OAuthError(format!("Failed to retrieve token: {}", e))),
    }
}

pub fn delete_token() -> Result<()> {
    let entry = keyring::Entry::new("adnt-net-edge", "github_token")
        .map_err(|e| GatewayError::OAuthError(format!("Failed to create keyring entry: {}", e)))?;

    entry
        .delete_credential()
        .map_err(|e| GatewayError::OAuthError(format!("Failed to delete token: {}", e)))?;

    tracing::info!("GitHub token removed from system keyring");
    Ok(())
}
