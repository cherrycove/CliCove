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

### CLI 用途资料

每个 `service-catalog.json` 条目都必须维护以下资料。「已安装」、可信安装目录和本机 CLI 详情使用同一份数据展示用途和详情弹框，无需在前端另写一份。

| 字段 | 填写内容 |
| --- | --- |
| `description` | 一句话用途，直接显示在安装列表中 |
| `introduction` | 详细介绍：所属产品、主要能力和适用场景；用 `\n\n` 分段 |
| `documentationUrl` | 已核实的官方说明页、官方文档或官方代码仓库的完整 HTTPS 链接 |

新增或修改条目时，同时核对介绍与官方说明。目录校验会拒绝缺失字段、空白介绍，以及不合法或非 HTTPS 的链接；链接是否属于官方仍需维护者核实。这些资料字段属于内置目录，不改变自定义适配器的配置格式。

只补充已支持工具的用途资料时，可以省略 `commands` 和 `packages`；这种条目仅提供介绍，不参与扫描识别，也不新增安装入口。`id` 与已有适配器 ID 一致；无适配器的工具使用明确的命令名（如 `aliyun`）。已识别的服务/适配器身份优先于命令名，避免把通用别名关联到其他产品。

新增内置适配器时，也必须补上对应资料条目，测试会检查覆盖情况。

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
- 安装列表是否显示用途，点击名称或“了解用途”能否查看详细介绍并打开官方说明。

提交 Pull Request 时附上工具官网、安装方式和测试环境。
