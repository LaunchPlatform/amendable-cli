mod common;

use std::fs;

use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;
use tempfile::TempDir;

use common::MockApiServer;
use common::SAMPLE_TOKEN;
use common::SAMPLE_USERNAME;
use common::SAMPLE_WEBHOOK_ID;

struct Harness {
    _dir: TempDir,
    config: std::path::PathBuf,
}

impl Harness {
    fn logged_in(server: &MockApiServer) -> Self {
        let dir = TempDir::new().expect("tempdir");
        let config = dir.path().join("config.toml");
        fs::write(
            &config,
            format!(
                "api_url = \"{}\"\napp_url = \"https://amendable.io\"\ntoken = \"{SAMPLE_TOKEN}\"\nusername = \"{SAMPLE_USERNAME}\"\n",
                server.url
            ),
        )
        .expect("write config");
        Self { _dir: dir, config }
    }

    fn empty(server: &MockApiServer) -> Self {
        let dir = TempDir::new().expect("tempdir");
        let config = dir.path().join("config.toml");
        fs::write(&config, format!("api_url = \"{}\"\n", server.url)).expect("write config");
        Self { _dir: dir, config }
    }

    fn cmd(&self) -> Command {
        let mut cmd = Command::cargo_bin("amendable").expect("binary");
        cmd.env("AMENDABLE_CONFIG", &self.config)
            .env_remove("AMENDABLE_TOKEN")
            .env_remove("AMENDABLE_API_URL")
            .env_remove("AMENDABLE_APP_URL")
            .env_remove("AMENDABLE_USERNAME");
        cmd
    }
}

#[test]
fn repo_list() {
    let server = MockApiServer::start();
    server.grant();
    let harness = Harness::logged_in(&server);
    harness
        .cmd()
        .args(["repo", "list", "--json"])
        .assert()
        .success()
        .stdout(contains("agent-workspace"));
}

#[test]
fn repo_create() {
    let server = MockApiServer::start();
    server.grant();
    let harness = Harness::logged_in(&server);
    harness
        .cmd()
        .args(["repo", "create", "new-repo", "--json"])
        .assert()
        .success()
        .stdout(contains("new-repo").and(contains(SAMPLE_USERNAME)));
}

#[test]
fn repo_get_and_url() {
    let server = MockApiServer::start();
    server.grant();
    let harness = Harness::logged_in(&server);
    harness
        .cmd()
        .args(["repo", "get", "agent-workspace", "--json"])
        .assert()
        .success()
        .stdout(contains("agent-workspace"));
    harness
        .cmd()
        .args(["repo", "url", "demo/agent-workspace"])
        .assert()
        .success()
        .stdout(contains("https://amendable.io/r/demo/agent-workspace.git"));
}

#[test]
fn repo_delete() {
    let server = MockApiServer::start();
    server.grant();
    let harness = Harness::logged_in(&server);
    harness
        .cmd()
        .args(["repo", "delete", "agent-workspace", "--yes"])
        .assert()
        .success();
    harness
        .cmd()
        .args(["repo", "list", "--json"])
        .assert()
        .success()
        .stdout(contains("agent-workspace").not());
}

#[test]
fn whoami_usage() {
    let server = MockApiServer::start();
    server.grant();
    let harness = Harness::logged_in(&server);
    harness
        .cmd()
        .args(["whoami", "--json"])
        .assert()
        .success()
        .stdout(contains("\"used\": 1"));
}

#[test]
fn token_create_and_list() {
    let server = MockApiServer::start();
    server.grant();
    let harness = Harness::logged_in(&server);
    harness
        .cmd()
        .args([
            "token",
            "create",
            "--name",
            "ci",
            "--grants",
            "GIT_HTTP_READ,GIT_HTTP_WRITE,API_REPOS_WRITE",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("NEWTOKENSHOWNONCE"));
    harness
        .cmd()
        .args(["token", "list", "--json"])
        .assert()
        .success()
        .stdout(contains("GIT_HTTP_WRITE"));
}

#[test]
fn webhook_create_ping_delete() {
    let server = MockApiServer::start();
    server.grant();
    let harness = Harness::logged_in(&server);
    harness
        .cmd()
        .args([
            "webhook",
            "create",
            "https://example.com/hooks/amendable",
            "--events",
            "push,ping",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("webhook-secret-shown-once"));
    harness
        .cmd()
        .args(["webhook", "ping", SAMPLE_WEBHOOK_ID])
        .assert()
        .success();
    harness
        .cmd()
        .args(["webhook", "delete", SAMPLE_WEBHOOK_ID])
        .assert()
        .success();
}

#[test]
fn login_polls_until_granted() {
    let server = MockApiServer::start();
    let dir = TempDir::new().expect("tempdir");
    let config = dir.path().join("config.toml");
    fs::write(
        &config,
        format!(
            "api_url = \"{}\"\napp_url = \"https://amendable.io\"\n",
            server.url
        ),
    )
    .expect("write config");
    server.grant();
    Command::cargo_bin("amendable")
        .expect("binary")
        .args(["login", "--hostname", "testhost", "--poll-seconds", "0.01"])
        .env("AMENDABLE_CONFIG", &config)
        .env_remove("AMENDABLE_TOKEN")
        .env_remove("AMENDABLE_API_URL")
        .env_remove("AMENDABLE_APP_URL")
        .env_remove("AMENDABLE_USERNAME")
        .assert()
        .success();
    let saved = fs::read_to_string(&config).expect("read config");
    assert!(saved.contains(SAMPLE_TOKEN), "{saved}");
    assert!(saved.contains(SAMPLE_USERNAME), "{saved}");
}

#[test]
fn oidc_exchange() {
    let server = MockApiServer::start();
    server.grant();
    let harness = Harness::logged_in(&server);
    harness
        .cmd()
        .args([
            "oidc-exchange",
            "--id-token",
            "header.payload.sig",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains(SAMPLE_TOKEN).and(contains("expires_at")));
}

#[test]
fn missing_token_fails() {
    let server = MockApiServer::start();
    server.grant();
    let harness = Harness::empty(&server);
    harness
        .cmd()
        .args(["repo", "list"])
        .assert()
        .failure()
        .stderr(contains("Not logged in"));
}

#[test]
fn help_omits_staging() {
    Command::cargo_bin("amendable")
        .expect("binary")
        .arg("--help")
        .assert()
        .success()
        .stdout(contains("--staging").not())
        .stdout(contains("stage.amendable.io").not())
        .stdout(contains("stage.api.amendable.io").not());
}

#[test]
fn hidden_staging_flag_is_accepted() {
    Command::cargo_bin("amendable")
        .expect("binary")
        .args(["--staging", "--help"])
        .assert()
        .success();
}

#[test]
fn git_credential_get() {
    let server = MockApiServer::start();
    server.grant();
    let harness = Harness::logged_in(&server);
    harness
        .cmd()
        .args(["git-credential", "get"])
        .write_stdin("protocol=https\nhost=amendable.io\n\n")
        .assert()
        .success()
        .stdout(contains(format!("username={SAMPLE_USERNAME}")))
        .stdout(contains(format!("password={SAMPLE_TOKEN}")));
}

#[test]
fn git_credential_store_is_ignored() {
    let server = MockApiServer::start();
    server.grant();
    let harness = Harness::logged_in(&server);
    harness
        .cmd()
        .args(["git-credential", "store"])
        .write_stdin("protocol=https\nhost=amendable.io\n\n")
        .assert()
        .success();
}
