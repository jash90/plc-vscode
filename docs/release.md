# Release and Marketplace publishing

This document describes how the PLC VS Code extension is packaged and published.

## Packaging path

The extension is packaged with [`@vscode/vsce`](https://github.com/microsoft/vscode-vsce):

```bash
cd editors/vscode
npm ci
npm run package   # vsce package --no-dependencies -> plc-vscode-<version>.vsix
```

The bundled language server binary is produced from the Rust workspace
(`cargo build --release --package plc_lsp_server --bin plc-lsp-server`) and is
shipped with the extension (see PLC-48).

## Automated release workflow

`.github/workflows/release.yml` runs on a `v*` tag push and:

runs a **matrix over target platforms** (macOS arm64/x64, Linux x64/arm64,
Windows x64) and for each:

1. builds the release `plc-lsp-server` and `plc` binaries for that target,
2. bundles them into `editors/vscode/server/` (`.exe` on Windows),
3. type-checks + runs the contract test, then packages a platform VSIX with
   `vsce package --target <vsce-platform>` (e.g. `darwin-arm64`, `linux-x64`,
   `win32-x64`),
4. uploads each `plc-vscode-<platform>.vsix` to the GitHub release,
5. publishes each platform VSIX to the Marketplace **iff** the `VSCE_PAT`
   secret is configured (`vsce publish --target …`).

The Marketplace serves the matching platform build automatically, so a user
installs one extension that already contains the right native server — no Rust
toolchain required. When **installed** (Production mode) the extension runs the
bundled `server/plc-lsp-server`; in development it falls back to `cargo run`
against the workspace.

## Release checklist

- [ ] Update `editors/vscode/package.json` `version` (semver).
- [ ] Update the changelog with user-facing changes for the version.
- [ ] Ensure CI (`.github/workflows/ci.yml`) is green on `main`.
- [ ] Verify the bundled server binary builds in release mode.
- [ ] `npm run package` succeeds locally and the `.vsix` installs/activates.
- [ ] Tag the release: `git tag vX.Y.Z && git push origin vX.Y.Z`.
- [ ] Confirm the release workflow uploaded the `.vsix` artifact.
- [ ] Confirm Marketplace publish (if `VSCE_PAT` configured) or publish manually.

## Marketplace publishing prerequisites

- A Visual Studio Marketplace **publisher** matching `publisher` in
  `package.json` (`raccoonsoftware`).
- A **Personal Access Token (PAT)** with Marketplace *Manage* scope from Azure
  DevOps, stored as the `VSCE_PAT` repository secret.
- Required `package.json` metadata: `publisher`, `name`, `version`, `engines.vscode`,
  `repository`, and an icon/README for the Marketplace listing.
- `vsce` validates the manifest at publish time; resolve any reported warnings
  (missing repository, license, etc.) before tagging.
