# VCorp Goose API customization

Bundled declarative provider: `vcorp` (OpenAI-compatible).

## Defaults

- Provider: `vcorp`
- Base URL env: `VCORP_BASE_URL` (default `https://api.vcorp.ai/v1/chat/completions`)
- API key env: `VCORP_API_KEY`
- Model: `gemini-3.8-flash-high`

## Distribution artifacts

Keep `init-config.yaml` next to the binary. First launch applies it when the user has no existing config.

| Platform | Package | Binary |
|---|---|---|
| Windows x64 CLI | `dist/windows-x86_64/goose-vcorp-windows-x86_64.zip` (60 MB) | `goose.exe` 164 MB |
| Windows x64 GUI | `dist/windows-x86_64-gui/Goose-vcorp-win32-x64.zip` (226 MB) | unpack and run `Goose.exe` |
| Linux x64 CLI | `dist/linux-x86_64/goose-vcorp-linux-x86_64.tar.gz` (85 MB) | `goose` 244 MB |

Local GUI (already unpacked): `ui/desktop/out/Goose-win32-x64/Goose.exe`

Windows is a MinGW (`windows-gnu`) build: VCorp is bundled; `local-inference` and `code-mode` are off because those need MSVC + prebuilt V8. Linux is a full default release build.

Rebuild:

```powershell
# Windows CLI
.\distro\vcorp\build-windows.ps1

# Windows GUI (needs the CLI goose.exe first)
.\distro\vcorp\build-windows-gui.ps1
```

```bash
# Linux (WSL or native)
./distro/vcorp/build-linux.sh
./distro/vcorp/package-linux.sh
```

Official-parity Windows (`x86_64-pc-windows-msvc`, local models + code-mode) needs Visual Studio 2022 Build Tools, then:

```powershell
just release-windows
```

## Configure after install

```powershell
# Windows PowerShell (User scope)
[Environment]::SetEnvironmentVariable("VCORP_API_KEY", "your-key", "User")
# optional override:
# [Environment]::SetEnvironmentVariable("VCORP_BASE_URL", "https://api.vcorp.ai/v1/chat/completions", "User")

goose configure
# pick VCorp, or:
goose run --provider vcorp --model gemini-3.8-flash-high -t "ping"
```

## Files touched

- `crates/goose-providers/src/declarative/definitions/vcorp.json`
- `crates/goose-providers/src/declarative.rs` (register `vcorp`)
- `init-config.yaml` / `distro/vcorp/init-config.yaml`
