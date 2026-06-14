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

1. builds the release language-server binary,
2. installs extension dependencies and type-checks,
3. packages the `.vsix`,
4. uploads the `.vsix` to the GitHub release,
5. publishes to the Marketplace **iff** the `VSCE_PAT` secret is configured.

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
