# 适配器开发

为 CliCove 添加 CLI 的识别、版本查询、诊断和管理操作。

## 添加自定义适配器

1. 在 CliCove「设置」中点击「打开适配器目录」。
2. 将 [adapter.example.json](adapter.example.json) 复制到该目录，重命名为你的工具名称。
3. 按工具的实际命令修改配置，不支持的操作填 `null`。
4. 返回设置，点击「刷新」→「审查并信任」，核对命令后确认。
5. 重新扫描本机 CLI。

修改适配器文件后，需要重新审查并信任。

### 配置字段

| 字段 | 填写内容 |
| --- | --- |
| `id` / `displayName` | 唯一标识和显示名称 |
| `names` | 工具的命令名及别名 |
| `category` | 工具分类，如 `ai-agent`、`collaboration`、`cloud-service`、`observability` |
| `versionCommands` | 只读版本命令的参数数组，如 `[["--version"]]` |
| `update` / `diagnose` / `auth` | 更新、诊断、登录命令；`executable` 为 `{self}` 时使用工具自身 |
| `configPaths` / `configCommand` | 配置文件路径或配置命令 |
| `dependencies` | 运行依赖，如 `["node"]` |
| `matchers` | 用路径或包名区分同名命令 |

命令参数按项填写 `args`，例如 `["status", "--json"]`。配置路径可使用 `~`。`id` 和命令名不能与现有适配器冲突，JSON 文件须为 UTF-8 且不超过 128 KiB。

## 添加内置支持

- **工具识别与安装目录**：在 [service-catalog.json](../src-tauri/service-catalog.json) 中添加命令名、分类和包标识。
- **管理操作**：在 [adapters.rs](../src-tauri/src/adapters.rs) 的 `builtin_adapters()` 中添加适配器，按需配置版本、登录、诊断及更新命令。
- **服务分组**：在 [resources.rs](../src-tauri/src/resources.rs) 的 `provider_id_for()` 和 `provider_for()` 中添加服务映射。
- **配置预览**：在 `config_preview_fields()` 中添加允许展示的字段，排除令牌、密钥和密码。

## 测试与提交

```bash
npm run check
npm test
cargo test --manifest-path src-tauri/Cargo.toml
```

在应用中检查：

- 扫描结果中的名称、分类、版本和安装来源是否正确。
- 已声明的登录、诊断和配置入口是否可用。
- 更新操作的命令预览是否符合工具的官方用法。

提交 Pull Request 时附上工具官网、安装方式和测试环境。
