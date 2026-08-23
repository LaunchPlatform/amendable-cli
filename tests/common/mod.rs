use std::io::Cursor;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;

use serde_json::json;
use serde_json::Value;
use tiny_http::Header;
use tiny_http::Method;
use tiny_http::Request;
use tiny_http::Response;
use tiny_http::Server;
use tiny_http::StatusCode;

pub const SAMPLE_TOKEN: &str = "5Q2exampleAccessTokenForTestsOnly";
pub const SAMPLE_USERNAME: &str = "demo";
pub const SAMPLE_REPO: &str = "agent-workspace";
pub const SAMPLE_WEBHOOK_ID: &str = "22222222-2222-2222-2222-222222222222";
pub const SAMPLE_WEBHOOK_SECRET: &str = "webhook-secret-shown-once";
pub const SAMPLE_AUTH_ID: &str = "33333333-3333-3333-3333-333333333333";
pub const SAMPLE_AUTH_CODE: &str = "AB12-CD34";
pub const SAMPLE_AUTH_SECRET: &str = "auth-session-secret";

pub struct MockState {
    pub granted: bool,
    pub repositories: Vec<Value>,
    pub webhooks: Vec<Value>,
    pub tokens: Vec<Value>,
    pub usage: Value,
}

impl Default for MockState {
    fn default() -> Self {
        Self {
            granted: false,
            repositories: vec![json!({
                "username": SAMPLE_USERNAME,
                "name": SAMPLE_REPO,
                "active": true,
                "storage_source": "platform",
            })],
            webhooks: Vec::new(),
            tokens: Vec::new(),
            usage: json!({
                "repos": {"used": 1, "quota": 5},
                "storage_bytes": {"used": 1024, "quota": 1073741824_i64},
                "transfer_bytes": {"used": 2048, "quota": 5368709120_i64},
                "ingress_bytes": {"used": 512, "quota": null},
            }),
        }
    }
}

pub struct MockApiServer {
    pub url: String,
    state: Arc<Mutex<MockState>>,
    running: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl MockApiServer {
    pub fn start() -> Self {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind mock api");
        let addr = listener.local_addr().expect("local addr");
        let url = format!("http://{addr}");
        let server = Server::from_listener(listener, None).expect("tiny_http server");
        let state = Arc::new(Mutex::new(MockState::default()));
        let running = Arc::new(AtomicBool::new(true));
        let thread_state = Arc::clone(&state);
        let thread_running = Arc::clone(&running);
        let thread = thread::spawn(move || {
            while thread_running.load(Ordering::SeqCst) {
                match server.recv_timeout(Duration::from_millis(50)) {
                    Ok(Some(request)) => handle(&thread_state, request),
                    Ok(None) => {}
                    Err(_) => break,
                }
            }
        });
        Self {
            url,
            state,
            running,
            thread: Some(thread),
        }
    }

    pub fn grant(&self) {
        self.state.lock().expect("state").granted = true;
    }
}

impl Drop for MockApiServer {
    fn drop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn handle(state: &Arc<Mutex<MockState>>, mut request: Request) {
    let url = request.url().to_string();
    let (path, query) = split_url(&url);
    let method = request.method().clone();
    let token = header_value(&request, "access-token");
    let body = read_json(&mut request);

    let response = match method {
        Method::Get => do_get(state, &path, &query, token.as_deref()),
        Method::Post => do_post(state, &path, token.as_deref(), body),
        Method::Delete => do_delete(state, &path, token.as_deref()),
        _ => json_response(404, json!({"detail": "Not found"})),
    };
    let _ = request.respond(response);
}

fn do_get(
    state: &Arc<Mutex<MockState>>,
    path: &str,
    query: &[(String, String)],
    token: Option<&str>,
) -> Response<Cursor<Vec<u8>>> {
    if path.starts_with("/v1/auth/sessions/") && path.ends_with("/poll") {
        let secret = query
            .iter()
            .find(|(k, _)| k == "secret_token")
            .map(|(_, v)| v.as_str());
        if secret != Some(SAMPLE_AUTH_SECRET) {
            return json_response(404, json!({"detail": "Auth session not found"}));
        }
        let granted = state.lock().expect("state").granted;
        if !granted {
            return json_response(
                202,
                json!({"code": "try_again", "message": "still waiting for auth"}),
            );
        }
        return json_response(
            200,
            json!({
                "token": SAMPLE_TOKEN,
                "repositories": [format!("{SAMPLE_USERNAME}/{SAMPLE_REPO}")],
            }),
        );
    }
    if !require_token(token) {
        return unauthorized();
    }
    let state = state.lock().expect("state");
    if path == "/v1/repositories" {
        return json_response(200, json!({"repositories": state.repositories}));
    }
    if path == "/v1/account/usage" {
        return json_response(200, state.usage.clone());
    }
    if path == "/v1/access-tokens" {
        return json_response(200, json!({"access_tokens": state.tokens}));
    }
    if path == "/v1/webhooks" {
        return json_response(200, json!({"webhooks": state.webhooks}));
    }
    if path.starts_with("/v1/repos/") && path.ends_with("/branches") {
        return json_response(
            200,
            json!({
                "default_branch": "main",
                "branches": []
            }),
        );
    }
    if path.starts_with("/v1/repos/") {
        let parts: Vec<&str> = path.split('/').collect();
        if parts.len() >= 5 {
            let username = parts[3];
            let name = parts[4];
            if let Some(repo) = state
                .repositories
                .iter()
                .find(|repo| repo["username"] == username && repo["name"] == name)
            {
                return json_response(200, repo.clone());
            }
            return json_response(404, json!({"detail": "Repo not found"}));
        }
    }
    if path.contains("/deliveries") {
        return json_response(200, json!({"deliveries": []}));
    }
    json_response(404, json!({"detail": "Not found"}))
}

fn do_post(
    state: &Arc<Mutex<MockState>>,
    path: &str,
    token: Option<&str>,
    body: Value,
) -> Response<Cursor<Vec<u8>>> {
    if path == "/v1/auth/sessions" {
        let hostname = body
            .get("hostname")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        return json_response(
            201,
            json!({
                "id": SAMPLE_AUTH_ID,
                "code": SAMPLE_AUTH_CODE,
                "auth_url": format!(
                    "https://amendable.io/access-tokens/create?auth_session_id={SAMPLE_AUTH_ID}"
                ),
                "secret_token": SAMPLE_AUTH_SECRET,
                "hostname": hostname,
            }),
        );
    }
    if path == "/v1/oidc/token" {
        if body.get("id_token").and_then(Value::as_str).is_none() {
            return json_response(401, json!({"detail": "Invalid ID token"}));
        }
        return json_response(
            200,
            json!({
                "token": SAMPLE_TOKEN,
                "token_type": "access-token",
                "expires_at": "2026-08-23T01:30:00+00:00",
            }),
        );
    }
    if !require_token(token) {
        return unauthorized();
    }
    let mut state = state.lock().expect("state");
    if path == "/v1/repositories" {
        let name = body["name"].as_str().unwrap_or("unnamed");
        let repo = json!({
            "username": SAMPLE_USERNAME,
            "name": name,
            "active": true,
            "description": body.get("description"),
            "storage_source": "platform",
            "storage_bucket_id": body.get("storage_bucket_id"),
        });
        state.repositories.push(repo.clone());
        return json_response(201, repo);
    }
    if path == "/v1/access-tokens" {
        let created = json!({
            "id": "44444444-4444-4444-4444-444444444444",
            "name": body.get("name"),
            "scope": body.get("scope"),
            "grants": body.get("grants"),
            "token": "NEWTOKENSHOWNONCE",
            "repositories": [],
        });
        let mut listed = created.clone();
        listed.as_object_mut().expect("object").remove("token");
        state.tokens.push(listed);
        return json_response(201, created);
    }
    if path == "/v1/webhooks" {
        let created = json!({
            "id": SAMPLE_WEBHOOK_ID,
            "url": body.get("url"),
            "events": body.get("events").cloned().unwrap_or_else(|| json!(["push"])),
            "active": body.get("active").and_then(Value::as_bool).unwrap_or(true),
            "secret": SAMPLE_WEBHOOK_SECRET,
        });
        let mut listed = created.clone();
        listed.as_object_mut().expect("object").remove("secret");
        state.webhooks.push(listed);
        return json_response(201, created);
    }
    if path.ends_with("/ping") {
        return json_response(
            202,
            json!({
                "event_id": "55555555-5555-5555-5555-555555555555",
                "delivery_id": "66666666-6666-6666-6666-666666666666",
            }),
        );
    }
    json_response(404, json!({"detail": "Not found"}))
}

fn do_delete(
    state: &Arc<Mutex<MockState>>,
    path: &str,
    token: Option<&str>,
) -> Response<Cursor<Vec<u8>>> {
    if !require_token(token) {
        return unauthorized();
    }
    let mut state = state.lock().expect("state");
    if path.starts_with("/v1/repos/") {
        let parts: Vec<&str> = path.split('/').collect();
        let username = parts[3];
        let name = parts[4];
        state
            .repositories
            .retain(|repo| !(repo["username"] == username && repo["name"] == name));
        return empty(204);
    }
    if path.starts_with("/v1/access-tokens/") {
        let token_id = path.rsplit('/').next().unwrap_or("");
        state.tokens.retain(|item| item["id"] != token_id);
        return empty(204);
    }
    if path.starts_with("/v1/webhooks/") {
        let webhook_id = path.rsplit('/').next().unwrap_or("");
        state.webhooks.retain(|item| item["id"] != webhook_id);
        return empty(204);
    }
    json_response(404, json!({"detail": "Not found"}))
}

fn require_token(token: Option<&str>) -> bool {
    token == Some(SAMPLE_TOKEN)
}

fn unauthorized() -> Response<Cursor<Vec<u8>>> {
    json_response(401, json!({"detail": "Unauthorized"}))
}

fn json_response(status: u16, payload: Value) -> Response<Cursor<Vec<u8>>> {
    let body = serde_json::to_vec(&payload).expect("json");
    Response::from_data(body)
        .with_status_code(StatusCode(status))
        .with_header(Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap())
}

fn empty(status: u16) -> Response<Cursor<Vec<u8>>> {
    Response::from_data(Vec::new()).with_status_code(StatusCode(status))
}

fn split_url(url: &str) -> (String, Vec<(String, String)>) {
    match url.split_once('?') {
        Some((path, query)) => {
            let pairs = query
                .split('&')
                .filter_map(|pair| {
                    let (key, value) = pair.split_once('=')?;
                    Some((key.to_string(), value.to_string()))
                })
                .collect();
            (path.to_string(), pairs)
        }
        None => (url.to_string(), Vec::new()),
    }
}

fn header_value(request: &Request, name: &str) -> Option<String> {
    request.headers().iter().find_map(|header| {
        if header.field.as_str().as_str().eq_ignore_ascii_case(name) {
            Some(header.value.as_str().to_string())
        } else {
            None
        }
    })
}

fn read_json(request: &mut Request) -> Value {
    let mut buf = String::new();
    let _ = std::io::Read::read_to_string(request.as_reader(), &mut buf);
    if buf.trim().is_empty() {
        json!({})
    } else {
        serde_json::from_str(&buf).unwrap_or_else(|_| json!({}))
    }
}
