# Agent notes for amendable-cli

Rust CLI (`amendable`) for the Amendable API.

## Product facts (do not invent others)

- Site + Git HTTPS: `https://amendable.io`
- API: `https://api.amendable.io` header `access-token`
- Clone: `https://amendable.io/r/<username>/<repo>.git`
- Git password is the access token. There is no SSH.
- Command name is `amendable`. Package name is `amendable-cli`.

Staging is a config override for Amendable developers, not a different command set:

- `amendable --staging` / `amendable login --staging`
- `AMENDABLE_API_URL=https://stage.api.amendable.io`
- `AMENDABLE_APP_URL=https://stage.amendable.io`

Do not document those URL env vars in customer docs (`../amendable-docs`). Hardcode production hosts there.

Config file: `~/.config/amendable/config.toml` (mode `0600`).

Authoritative API behavior lives in `../amendable-web`. Customer docs live in `../amendable-docs`. If the CLI disagrees with the API, fix the CLI. If the docs disagree with the CLI, fix the docs.

## Commands

Keep the public command surface stable. Docs and examples call `amendable`, not `amendable-cli`.

## Tests

```bash
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```
