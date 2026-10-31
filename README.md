# 旅游照片 / 视频整理

本地优先的跨平台桌面工具：扫描旅游照片、RAW 和视频，按拍摄时间整理为
`年 / 年月 / 文件名`，按字节与视觉相似度识别重复项，并在用户确认后安全移动。

核心使用 **Rust + Tauri 2**，界面使用 **Vue 3 + TypeScript + Vite**，不依赖 Python。
支持 macOS、Windows 和 Linux。

## 安全边界

- 扫描和计划阶段只读，默认 dry-run。
- 重复文件只移动到 `_duplicates/`，不会自动硬删除。
- 目标冲突时追加 ` (1)`、` (2)` 后缀，绝不覆盖。
- 每次执行写入 JSON 可回滚日志，可按相反路径恢复。
- 彻底删除隔离区需要两次确认，并会明确提示本次执行不再可自动回滚。

## 整理结果

```text
<目标根目录>/
  2024/
    2024-05/
      原文件名.jpg
      同名但内容不同 (1).jpg
  unresolved/
    无法解析日期的文件.jpg
  _duplicates/
    2024/
      2024-05/
        重复文件.jpg
```

拍摄日期依次尝试 EXIF、视频元数据、文件创建/修改时间和文件名。RAW/HEIC/视频在
exiftool 或 ffprobe 可用时读取更完整的元数据；工具缺失时会降级，但不会丢文件。

## 开发环境

- Node.js >= 20
- pnpm
- Rust stable
- 各平台 Tauri 2 系统依赖

```bash
pnpm install
pnpm tauri dev
```

仅构建前端：

```bash
pnpm build
```

## 测试

```bash
# Rust 单元测试
cargo test --manifest-path src-tauri/Cargo.toml --lib

# 前端 store 单元测试
pnpm exec vitest run

# 完整后端验收：扫描 → 去重 → 计划 → 执行 → 幂等复扫 → 回滚
./scripts/e2e-smoke.sh

# Windows PowerShell
.\scripts\e2e-smoke.ps1
```

## 打包

```bash
pnpm tauri build
```

图标源文件为 `app-icon.svg`，重新生成全套图标：

```bash
pnpm tauri icon app-icon.svg
```

仓库内置 GitHub Actions：

- `.github/workflows/ci.yml`：macOS、Windows、Ubuntu 上构建并运行测试。
- `.github/workflows/release.yml`：推送 `v*` 标签后，在三个平台生成安装包并创建
  draft release。

Tauri 安装包应在目标操作系统构建，不能在 macOS 上直接交叉生成 Windows 安装程序。

## 外部工具

| 工具 | 用途 | 缺失时 |
| --- | --- | --- |
| `exiftool` | HEIC、RAW、视频等元数据 | 尝试 ffprobe、文件时间、文件名 |
| `ffprobe` | 视频容器拍摄时间 | 尝试 exiftool、文件时间、文件名 |

应用启动时会检查 `PATH`、Homebrew 路径，以及 Windows 常见安装位置。也可以在
“规则配置”页手动选择可执行文件路径，配置保存在本机应用数据目录。

## 文档

- `PLAN.md`：产品目标、功能范围与验收标准。
- `ARCHITECTURE.md`：技术栈、模块、IPC 契约、线程模型与打包策略。
- `STRUCTURE.md`：目录结构与模块职责。

## 设计参考

- [qarmin/czkawka](https://github.com/qarmin/czkawka)：Rust 相似图/重复文件工具。
- [pkolaczk/fclones](https://github.com/pkolaczk/fclones)：大小分组与并行哈希思路。
- [ivandokov/phockup](https://github.com/ivandokov/phockup)：按 EXIF 日期归档照片。
- [tfeldmann/organize](https://github.com/tfeldmann/organize)：规则化整理和 dry-run 工作流。
