use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use std::collections::HashMap;

const DEFAULT_API_KEY_HEADER: &str = "X-API-Key";

#[derive(Clone, Debug, Default)]
pub struct Credentials {
    pub basic_username: Option<String>,
    pub basic_password: Option<String>,
    pub bearer_token: Option<String>,
    pub api_key: Option<String>,
    pub api_key_header: Option<String>,
    pub headers: HashMap<String, String>,
}

impl Credentials {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_basic(mut self, username: impl Into<String>, password: impl Into<String>) -> Self {
        self.basic_username = Some(username.into());
        self.basic_password = Some(password.into());
        self
    }

    pub fn with_bearer(mut self, token: impl Into<String>) -> Self {
        self.bearer_token = Some(token.into());
        self
    }

    pub fn with_api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }

    pub fn with_api_key_header(mut self, header: impl Into<String>) -> Self {
        self.api_key_header = Some(header.into());
        self
    }

    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }

    pub fn is_empty(&self) -> bool {
        self.basic_username.is_none()
            && self.basic_password.is_none()
            && self.bearer_token.is_none()
            && self.api_key.is_none()
            && self.api_key_header.is_none()
            && self.headers.is_empty()
    }

    pub fn header_map(&self) -> HashMap<String, String> {
        let mut headers = self.extra_headers();

        if let Some(ref api_key) = self.api_key {
            let header_name = self
                .api_key_header
                .as_deref()
                .unwrap_or(DEFAULT_API_KEY_HEADER);
            headers.insert(header_name.to_string(), api_key.clone());
        }

        match (self.basic_username.as_ref(), self.basic_password.as_ref()) {
            (Some(username), Some(password)) => {
                let credentials = format!("{}:{}", username, password);
                let encoded = BASE64.encode(credentials.as_bytes());
                headers.insert("Authorization".to_string(), format!("Basic {}", encoded));
            }
            _ => {
                if let Some(ref token) = self.bearer_token {
                    headers.insert("Authorization".to_string(), format!("Bearer {}", token));
                }
            }
        }

        headers
    }

    pub fn basic_auth(&self) -> Option<(String, String)> {
        match (self.basic_username.as_ref(), self.basic_password.as_ref()) {
            (Some(u), Some(p)) => Some((u.clone(), p.clone())),
            _ => None,
        }
    }

    pub fn bearer(&self) -> Option<String> {
        self.bearer_token.clone()
    }

    fn extra_headers(&self) -> HashMap<String, String> {
        self.headers
            .iter()
            .filter(|(k, v)| !k.is_empty() && !v.is_empty())
            .filter(|(k, _)| !k.eq_ignore_ascii_case("authorization"))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_credentials() {
        let creds = Credentials::new();
        assert!(creds.is_empty());
        assert!(creds.header_map().is_empty());
    }

    #[test]
    fn test_basic_auth() {
        let creds = Credentials::new().with_basic("user", "pass");
        let headers = creds.header_map();

        let auth = headers.get("Authorization").unwrap();
        assert!(auth.starts_with("Basic "));

        let (user, pass) = creds.basic_auth().unwrap();
        assert_eq!(user, "user");
        assert_eq!(pass, "pass");
    }

    #[test]
    fn test_bearer_token() {
        let creds = Credentials::new().with_bearer("secret-token");
        let headers = creds.header_map();

        assert_eq!(headers.get("Authorization").unwrap(), "Bearer secret-token");
        assert_eq!(creds.bearer().unwrap(), "secret-token");
    }

    #[test]
    fn test_api_key_default_header() {
        let creds = Credentials::new().with_api_key("my-api-key");
        let headers = creds.header_map();

        assert_eq!(headers.get("X-API-Key").unwrap(), "my-api-key");
    }

    #[test]
    fn test_api_key_custom_header() {
        let creds = Credentials::new()
            .with_api_key("my-api-key")
            .with_api_key_header("X-Custom-Key");
        let headers = creds.header_map();

        assert_eq!(headers.get("X-Custom-Key").unwrap(), "my-api-key");
    }

    #[test]
    fn test_basic_auth_overrides_bearer() {
        let creds = Credentials::new()
            .with_bearer("token")
            .with_basic("user", "pass");
        let headers = creds.header_map();

        let auth = headers.get("Authorization").unwrap();
        assert!(auth.starts_with("Basic "));
    }

    #[test]
    fn test_custom_headers_exclude_authorization() {
        let mut extra = HashMap::new();
        extra.insert("Authorization".to_string(), "ignored".to_string());
        extra.insert("X-Custom".to_string(), "value".to_string());

        let creds = Credentials {
            headers: extra,
            bearer_token: Some("token".to_string()),
            ..Default::default()
        };

        let headers = creds.header_map();
        assert_eq!(headers.get("Authorization").unwrap(), "Bearer token");
        assert_eq!(headers.get("X-Custom").unwrap(), "value");
    }
}
