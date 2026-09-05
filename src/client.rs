use serde_json::Value;

use crate::error::Error;

const USER_AGENT: &str = concat!("amendable-cli/", env!("CARGO_PKG_VERSION"));

pub struct Client {
    api_url: String,
    token: Option<String>,
    http: reqwest::blocking::Client,
}

impl Client {
    pub fn new(api_url: &str, token: Option<&str>) -> Result<Self, Error> {
        let http = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .user_agent(USER_AGENT)
            .build()?;
        Ok(Self {
            api_url: api_url.trim_end_matches('/').to_string(),
            token: token.map(str::to_string),
            http,
        })
    }

    fn headers(&self, auth: bool) -> Result<reqwest::header::HeaderMap, Error> {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::ACCEPT,
            reqwest::header::HeaderValue::from_static("application/json"),
        );
        if auth {
            let token = self.token.as_deref().ok_or(Error::NotLoggedIn)?;
            headers.insert(
                reqwest::header::AUTHORIZATION,
                reqwest::header::HeaderValue::from_str(&format!("Bearer {token}"))
                    .map_err(|err| Error::message(err.to_string()))?,
            );
        }
        Ok(headers)
    }

    pub fn request(
        &self,
        method: reqwest::Method,
        path: &str,
        auth: bool,
        json: Option<&Value>,
        query: &[(&str, String)],
        expected: &[u16],
    ) -> Result<Value, Error> {
        let url = format!("{}{path}", self.api_url);
        let mut builder = self.http.request(method, url).headers(self.headers(auth)?);
        if !query.is_empty() {
            builder = builder.query(query);
        }
        if let Some(body) = json {
            builder = builder.json(body);
        }
        let response = builder.send()?;
        let status = response.status().as_u16();
        if !expected.contains(&status) {
            let detail = match response.json::<Value>() {
                Ok(payload) => detail_from_body(&payload),
                Err(_) => "request failed".to_string(),
            };
            return Err(Error::api(status, detail));
        }
        if status == 204 {
            return Ok(Value::Null);
        }
        let bytes = response.bytes()?;
        if bytes.is_empty() {
            return Ok(Value::Null);
        }
        Ok(serde_json::from_slice(&bytes)?)
    }

    pub fn create_auth_session(&self, hostname: &str) -> Result<Value, Error> {
        self.request(
            reqwest::Method::POST,
            "/v1/auth/sessions",
            false,
            Some(&serde_json::json!({ "hostname": hostname })),
            &[],
            &[201],
        )
    }

    pub fn poll_auth_session(&self, session_id: &str, secret_token: &str) -> Result<Value, Error> {
        self.request(
            reqwest::Method::GET,
            &format!("/v1/auth/sessions/{session_id}/poll"),
            false,
            None,
            &[("secret_token", secret_token.to_string())],
            &[200, 202],
        )
    }

    pub fn list_repositories(&self) -> Result<Value, Error> {
        self.request(
            reqwest::Method::GET,
            "/v1/repositories",
            true,
            None,
            &[],
            &[200],
        )
    }

    pub fn create_repository(
        &self,
        name: &str,
        description: Option<&str>,
        storage_bucket_id: Option<&str>,
    ) -> Result<Value, Error> {
        let mut body = serde_json::Map::new();
        body.insert("name".into(), Value::String(name.to_string()));
        if let Some(description) = description.filter(|v| !v.is_empty()) {
            body.insert("description".into(), Value::String(description.to_string()));
        }
        if let Some(storage_bucket_id) = storage_bucket_id.filter(|v| !v.is_empty()) {
            body.insert(
                "storage_bucket_id".into(),
                Value::String(storage_bucket_id.to_string()),
            );
        }
        self.request(
            reqwest::Method::POST,
            "/v1/repositories",
            true,
            Some(&Value::Object(body)),
            &[],
            &[201],
        )
    }

    pub fn get_repository(&self, username: &str, name: &str) -> Result<Value, Error> {
        self.request(
            reqwest::Method::GET,
            &format!("/v1/repos/{username}/{name}"),
            true,
            None,
            &[],
            &[200],
        )
    }

    pub fn delete_repository(&self, username: &str, name: &str) -> Result<(), Error> {
        self.request(
            reqwest::Method::DELETE,
            &format!("/v1/repos/{username}/{name}"),
            true,
            None,
            &[],
            &[204],
        )?;
        Ok(())
    }

    pub fn get_usage(&self) -> Result<Value, Error> {
        self.request(
            reqwest::Method::GET,
            "/v1/account/usage",
            true,
            None,
            &[],
            &[200],
        )
    }

    pub fn list_access_tokens(&self) -> Result<Value, Error> {
        self.request(
            reqwest::Method::GET,
            "/v1/access-tokens",
            true,
            None,
            &[],
            &[200],
        )
    }

    pub fn create_access_token(
        &self,
        name: Option<&str>,
        scope: &str,
        grants: &[String],
        repository_ids: Option<&[String]>,
        allowed_branches: Option<&[String]>,
    ) -> Result<Value, Error> {
        let mut body = serde_json::json!({
            "name": name,
            "scope": scope,
            "grants": grants,
        });
        if let Some(ids) = repository_ids.filter(|v| !v.is_empty()) {
            body["repository_ids"] = serde_json::json!(ids);
        }
        if let Some(branches) = allowed_branches.filter(|v| !v.is_empty()) {
            body["allowed_branches"] = serde_json::json!(branches);
        }
        self.request(
            reqwest::Method::POST,
            "/v1/access-tokens",
            true,
            Some(&body),
            &[],
            &[201],
        )
    }

    pub fn delete_access_token(&self, token_id: &str) -> Result<(), Error> {
        self.request(
            reqwest::Method::DELETE,
            &format!("/v1/access-tokens/{token_id}"),
            true,
            None,
            &[],
            &[204],
        )?;
        Ok(())
    }

    pub fn list_webhooks(&self) -> Result<Value, Error> {
        self.request(
            reqwest::Method::GET,
            "/v1/webhooks",
            true,
            None,
            &[],
            &[200],
        )
    }

    pub fn create_webhook(&self, url: &str, events: &[String]) -> Result<Value, Error> {
        self.request(
            reqwest::Method::POST,
            "/v1/webhooks",
            true,
            Some(&serde_json::json!({
                "url": url,
                "events": events,
                "active": true,
            })),
            &[],
            &[201],
        )
    }

    pub fn ping_webhook(&self, webhook_id: &str) -> Result<Value, Error> {
        self.request(
            reqwest::Method::POST,
            &format!("/v1/webhooks/{webhook_id}/ping"),
            true,
            None,
            &[],
            &[202],
        )
    }

    pub fn delete_webhook(&self, webhook_id: &str) -> Result<(), Error> {
        self.request(
            reqwest::Method::DELETE,
            &format!("/v1/webhooks/{webhook_id}"),
            true,
            None,
            &[],
            &[204],
        )?;
        Ok(())
    }

    pub fn list_webhook_deliveries(
        &self,
        webhook_id: &str,
        include_payload: bool,
    ) -> Result<Value, Error> {
        self.request(
            reqwest::Method::GET,
            &format!("/v1/webhooks/{webhook_id}/deliveries"),
            true,
            None,
            &[(
                "include_payload",
                if include_payload {
                    "true".into()
                } else {
                    "false".into()
                },
            )],
            &[200],
        )
    }

    pub fn exchange_oidc_token(&self, id_token: &str) -> Result<Value, Error> {
        self.request(
            reqwest::Method::POST,
            "/v1/oidc/token",
            false,
            Some(&serde_json::json!({ "id_token": id_token })),
            &[],
            &[200],
        )
    }
}

fn detail_from_body(payload: &Value) -> String {
    match payload.get("detail") {
        Some(Value::String(text)) => text.clone(),
        Some(other) => other.to_string(),
        None => payload.to_string(),
    }
}
