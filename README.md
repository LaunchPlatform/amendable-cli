# amendable-cli

Command line client for [Amendable](https://amendable.io). The command name is `amendable`.

## Install

Download the archive for your OS from [GitHub Releases](https://github.com/LaunchPlatform/amendable-cli/releases/latest). Extract `amendable` and put it on `PATH`.

| Platform | Archive |
| --- | --- |
| Linux x86_64 | `amendable-x86_64-unknown-linux-musl.tar.gz` |
| Linux ARM64 | `amendable-aarch64-unknown-linux-musl.tar.gz` |
| macOS Apple Silicon | `amendable-aarch64-apple-darwin.tar.gz` |
| macOS Intel | `amendable-x86_64-apple-darwin.tar.gz` |
| Windows x86_64 | `amendable-x86_64-pc-windows-msvc.zip` |

Linux archives are static musl builds.

```bash
curl -fsSL -o amendable.tar.gz \
  https://github.com/LaunchPlatform/amendable-cli/releases/latest/download/amendable-x86_64-unknown-linux-musl.tar.gz
tar -xzf amendable.tar.gz amendable
sudo mv amendable /usr/local/bin/amendable
amendable login
amendable repo create hello
amendable repo clone hello
```

Talks to `https://api.amendable.io` by default.

### Cargo

Requires Rust 1.88+ ([rustup](https://rustup.rs/)).

```bash
cargo install --git https://github.com/LaunchPlatform/amendable-cli --locked
```

From a checkout:

```bash
git clone https://github.com/LaunchPlatform/amendable-cli.git
cd amendable-cli
cargo install --path . --locked
amendable --help
```

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
