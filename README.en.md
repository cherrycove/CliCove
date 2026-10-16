<img src="src/assets/clicove-mark.svg" width="72" height="72" alt="CliCove" />

# CliCove

[简体中文](README.md) | **English**

A local desktop workspace for your command-line tools.

Discover installed CLIs, check their status, manage installs and updates, and open terminal sessions in one window. Keep your AI coding tools, cloud CLIs, and collaboration tools within reach.

![CliCove CLI inventory and status panel](docs/screenshots/cli-inventory.png)

## Features

- **CLI inventory**: discover installed tools, browse by category, and inspect versions and installation sources.
- **Services and installations**: group tools by service and view multiple installations and available actions.
- **Install and update**: check for new versions, review and confirm operations, and verify the installed version afterward.
- **Status and diagnostics**: check dependencies, authentication, and common issues, with quick access to login and configuration actions.
- **Built-in terminal**: run CLIs inside the app and switch between multiple sessions.
- **App updates**: install a new CliCove version, remind later, or skip a release.
- **Local workspace**: no CliCove account required. Each CLI continues to manage its own login and credentials.

## Screenshots

| Services and installations | Install and update |
| --- | --- |
| ![CLI installations grouped by service](docs/screenshots/services.png) | ![Latest versions and CLI updates](docs/screenshots/updates.png) |

## CLI catalog

| Category | Example tools |
| --- | --- |
| AI and agents | Codex, Claude Code, Grok CLI, OpenCode, Trellis |
| Collaboration and business | Lark CLI, Meegle CLI |
| Cloud services | Aliyun, AWS CLI, Azure CLI, Google Cloud CLI, Volcengine CLI, Cloudflare Wrangler, Vercel CLI |
| Observability and operations | Owl |

Add more tools with [custom adapters](docs/ADAPTER_DEVELOPMENT.md) (guide in Chinese).

## Quick start

Install Node.js 22.12+, Rust stable, and the [Tauri 2 system prerequisites](https://v2.tauri.app/start/prerequisites/).

```bash
git clone https://github.com/cherrycove/CliCove.git
cd CliCove
npm ci
npm run tauri dev
```

For macOS and Windows.

## Keyboard shortcuts

| Shortcut | Action |
| --- | --- |
| `Cmd / Ctrl + K` | Search pages and CLIs |
| `Cmd / Ctrl + 1–5` | Switch between main pages |
| `/` | Focus the CLI search field |

## Contributing

[Open an issue](https://github.com/cherrycove/CliCove/issues) to report a bug or suggest an improvement, or send a pull request.

Good places to start include CLI adapters, UI improvements, documentation, and Windows validation. See the [adapter guide](docs/ADAPTER_DEVELOPMENT.md) (in Chinese) and [example configuration](docs/adapter.example.json).

Run the local checks:

```bash
npm run check
npm test
cargo test --manifest-path src-tauri/Cargo.toml
```

## License

[MIT](LICENSE)
