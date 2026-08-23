# amendable-cli

Command line client for [Amendable](https://amendable.io). The command name is `amendable`.

```bash
cargo install --git https://github.com/LaunchPlatform/amendable-cli --locked
amendable login
amendable repo create hello
amendable repo clone hello
```

Talks to `https://api.amendable.io` by default.

## Install from a checkout

```bash
git clone https://github.com/LaunchPlatform/amendable-cli.git
cd amendable-cli
cargo install --path . --locked
amendable --help
```

Requires Rust 1.85+ ([rustup](https://rustup.rs/)).

## Config

`~/.config/amendable/config.toml` (created on `login`, mode `0600`):

```toml
api_url = "https://api.amendable.io"
app_url = "https://amendable.io"
token = "..."
username = "yourname"
```

Override with `AMENDABLE_TOKEN`, `AMENDABLE_USERNAME`, or `AMENDABLE_CONFIG`.

## Tests

```bash
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

Product docs: https://docs.amendable.io/cli/install/
