# Agent notes for amendable-cli

Rust CLI (`amendable`) for the Amendable API.

## Product facts (do not invent others)

- Site + Git HTTPS: `https://amendable.io`
- API: `https://api.amendable.io` header `Authorization: Bearer`
- Clone: `https://amendable.io/r/<username>/<repo>.git`
- Git password is the access token. There is no SSH.
- Command name is `amendable`. Package name is `amendable-cli`.

Staging is a config override for Amendable developers, not a different command set:

- `amendable --staging` / `amendable login --staging`
- `AMENDABLE_API_URL=https://stage.api.amendable.io`
- `AMENDABLE_APP_URL=https://stage.amendable.io`

`--staging` is hidden from `amendable --help`. Keep it that way. Do not put staging hostnames, `--staging`, or `AMENDABLE_API_URL` / `AMENDABLE_APP_URL` in `README.md` or customer docs (`../amendable-docs`). Hardcode production hosts there. Staging URLs stay in this file and in `src/config.rs`.

Config file: `~/.config/amendable/config.toml` (mode `0600`).

Authoritative API behavior lives in `../amendable-web`. Customer docs live in `../amendable-docs`. If the CLI disagrees with the API, fix the CLI. If the docs disagree with the CLI, fix the docs.

## Commands

Keep the public command surface stable. Docs and examples call `amendable`, not `amendable-cli`.

## Tests

Do not add tautological tests that only assert a default host, constant, or
constructor dump equals the literal you just set. If changing an intended
default requires updating the test, and the test never exercises behavior,
omit it. Test env/file override, staging vs production selection, and CLI
contracts instead.

```bash
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

## Releases

Set `version` in `Cargo.toml`, commit on `master`, then push a matching `MAJOR.MINOR.PATCH` tag (`git tag 0.1.0 && git push origin 0.1.0`). GitHub Actions creates the GitHub Release and uploads `amendable` archives:

- Linux x86_64 and ARM64 (static musl)
- macOS Apple Silicon and Intel
- Windows x86_64

The tag must point at a commit on `master` and must match `Cargo.toml`. Do not create the GitHub Release in the UI first; the workflow creates it from the tag.
