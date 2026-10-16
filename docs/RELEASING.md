# 发布 CliCove

## 一次性配置

1. 保管 Tauri 更新签名私钥的离线备份。私钥不放入仓库，不随安装包分发；后续版本继续使用同一把密钥。
2. 在 GitHub 仓库的 **Settings → Secrets and variables → Actions** 添加 `TAURI_SIGNING_PRIVATE_KEY`，值为私钥文件全文。有密码时另设 `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`。
3. 公钥配置在 `src-tauri/tauri.conf.json` 的 `plugins.updater.pubkey`。私钥和公钥必须配对。

macOS 使用 ad-hoc 签名；Tauri 更新签名用于验证更新包，与系统代码签名分别配置。

## 发布新版本

1. 同步修改 `package.json`、`package-lock.json`、`src-tauri/Cargo.toml`、`src-tauri/Cargo.lock`、`src-tauri/tauri.conf.json` 的应用版本，提交并推送。
2. 在 **Actions → Build release → Run workflow** 输入版本标签，如 `v0.6.12`。也可执行：

   ```bash
   gh workflow run release.yml --ref main -f tag=v0.6.12
   ```

3. 等待全部平台构建成功。流程会创建草稿 Release，包含：

   | 平台 | 下载与安装 | 应用内更新 |
   | --- | --- | --- |
   | macOS Apple Silicon | `.dmg` | `.app.tar.gz` + `.sig` |
   | macOS Intel | `.dmg` | `.app.tar.gz` + `.sig` |
   | Windows x64 | `.msi`、`-setup.exe` | 对应安装包 + `.sig` |

   另外生成 `latest.json` 和 `SHA256SUMS`。缺少任一平台或签名时不会创建新草稿。

4. 从草稿下载安装包验证，编辑 Release 说明，再点击 **Publish release**，作为正式 Latest 版本发布。不要删除 `latest.json`、更新包或签名文件。
5. 上一版 CliCove 在下一次自动检查或手动检查时即可发现更新。

已发布的同名版本不会被工作流覆盖。需要重试时可以重跑草稿版本；正式版本的问题使用新的版本号发布。

## 首次启用与验证

- 0.6.12 是首个带应用更新功能的版本。更早版本需先手动安装一次。
- 在“设置 → CliCove 更新”中检查更新或关闭自动检查。自动检查默认开启，最多每 6 小时联网一次。
- “跳过此版本”会保存选择；手动检查仍会显示该版本。“稍后提醒”只关闭当前提示。
- 在已安装的旧版上验证下一版的检查、跳过、下载、安装与重启，并检查设置是否保留。Mac 两种架构、Windows MSI 与 EXE 安装路径分别验证。
- 终端仍运行时不能安装更新。结束会话后重试。
- 尚无正式 Release 时，检查会提示暂时无法获取更新信息。
- GitHub 无法连接或签名校验失败时，显示错误并允许重试，不执行安装。

本地开发和普通打包不需要私钥。发布工作流单独开启 `createUpdaterArtifacts` 并从 GitHub Secrets 读取私钥。
