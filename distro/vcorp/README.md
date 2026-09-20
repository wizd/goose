# VCorp Goose

Custom Goose distro with a bundled OpenAI-compatible provider: `vcorp`.

## Defaults

| | |
|---|---|
| Provider | `vcorp` |
| Default model | `google/gemini-3.8-flash` |
| API key | `VCORP_API_KEY` |
| Base URL | `VCORP_BASE_URL` (default `https://api.vcorp.ai/v1/chat/completions`) |
| Model list | live `GET /v1/models` (`dynamic_models: true`) |

Keep `init-config.yaml` next to the binary. First launch applies it only when the user has no existing config (`GOOSE_PROVIDER`, `GOOSE_MODEL`, `smart_approve`, telemetry off).

Do not add a second custom provider named `vcorp`. The bundled one cannot be deleted; a file-based duplicate shows up as `custom_vcorp`.

## Packages

Outputs land in `distro/vcorp/dist/` (gitignored).

| Platform | Archive | Run |
|---|---|---|
| Windows x64 CLI | `windows-x86_64/goose-vcorp-windows-x86_64.zip` | `goose.exe` |
| Windows x64 GUI | `windows-x86_64-gui/Goose-vcorp-win32-x64.zip` | unpack, then `Goose.exe` |
| Linux x64 CLI | `linux-x86_64/goose-vcorp-linux-x86_64.tar.gz` | `goose` |

Unpack-only GUI (not zipped): `ui/desktop/out/Goose-win32-x64/Goose.exe`.

Windows is MinGW (`x86_64-pc-windows-gnu`): VCorp is bundled; `local-inference` and `code-mode` are off (those need MSVC + prebuilt V8). Linux is a full default release build.

Official-parity Windows (`x86_64-pc-windows-msvc`) needs Visual Studio 2022 Build Tools, then `just release-windows`.

## Rebuild

Scripts resolve the repo from their own path and restore the previous working directory when they finish. Invoke them from anywhere:

```powershell
# From the repo root, or with a full path from any directory
.\distro\vcorp\build-all.ps1
```

Default is incremental (`cargo` / `pnpm` reuse caches; Linux rsyncs into `~/goose-vcorp-build` and rebuilds).

```powershell
# Wipe build outputs, then rebuild everything
.\distro\vcorp\build-all.ps1 -Clean
```

`-Clean` does:

- `cargo +1.96.1-x86_64-pc-windows-gnu clean`
- delete `ui/desktop/out`, `ui/desktop/.vite`, staged `ui/desktop/src/bin/goose.exe`
- delete `distro/vcorp/dist`
- Linux: full `build-linux.sh` (recreate `~/goose-vcorp-build`) instead of incremental `rebuild-linux.sh`

It does **not** delete `node_modules` or the pnpm store.

Order: Windows CLI → Windows GUI → Linux CLI (WSL `Ubuntu` only; skipped if `wsl` is missing).

### Individual targets

```powershell
.\distro\vcorp\build-windows.ps1          # also accepts -Clean
.\distro\vcorp\build-windows-gui.ps1      # needs goose.exe from the CLI build
```

```bash
# Linux (WSL or native)
./distro/vcorp/build-linux.sh      # wipe ~/goose-vcorp-build, copy, cargo build
./distro/vcorp/rebuild-linux.sh    # incremental after the first full copy
./distro/vcorp/package-linux.sh    # stage + tar.gz into dist/linux-x86_64
```

## Configure after install

```powershell
[Environment]::SetEnvironmentVariable("VCORP_API_KEY", "your-key", "User")
# optional:
# [Environment]::SetEnvironmentVariable("VCORP_BASE_URL", "https://api.vcorp.ai/v1/chat/completions", "User")

goose configure
goose run --provider vcorp --model google/gemini-3.8-flash -t "ping"
```

Existing user config is **not** overwritten by `init-config.yaml`. Official Goose and this distro share the same user data:

- `%APPDATA%\Block\goose` — CLI / ACP (`config.yaml`, sessions)
- `%APPDATA%\Goose` — desktop (`settings.json`, including `recentModels`)

User-scope `GOOSE_PROVIDER` / `GOOSE_MODEL` environment variables override `config.yaml`. If they point at a deleted custom provider (`custom_vcorp`), Settings → Switch models appears to do nothing. Unset them, then restart Goose.

```powershell
[Environment]::SetEnvironmentVariable("GOOSE_PROVIDER", $null, "User")
[Environment]::SetEnvironmentVariable("GOOSE_MODEL", $null, "User")
```

## Source

- `crates/goose-providers/src/declarative/definitions/vcorp.json`
- `crates/goose-providers/src/declarative.rs` (register `vcorp`)
- `init-config.yaml` / `distro/vcorp/init-config.yaml`
- `CUSTOM_DISTROS.md`
