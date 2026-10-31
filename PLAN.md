# 旅游照片/视频整理与去重工具 — 计划任务书

> 本文档是交给执行 agent 的完整规格说明。目标是搭建一个带 GUI 的桌面工具，
> 把散乱、含重复的旅游照片和视频，按「年 / 月」归档，并识别、隔离重复文件。
> 执行 agent 请严格按照本规格实现，勿自行删减关键安全约束。

> 技术栈与系统架构（模块边界、IPC 契约、数据模型、线程模型）见同目录 `ARCHITECTURE.md`，
> 实施以 `ARCHITECTURE.md` 为基线；两文档冲突时以 `ARCHITECTURE.md` 为准。

## 1. 目标与范围

### 1.1 目标

- 扫描用户指定的一个或多个源目录（可递归），识别照片和视频。
- 依据「拍摄时间」把所有媒体文件归类到 `年 / 年月` 两级目录。
- 识别重复文件（字节级重复，以及视觉上相似的图片），只保留一个，其余隔离。
- 提供 GUI：选择目录 → 配置 → 扫描 → 预览整理计划 → 确认执行 → 查看结果与回滚。

### 1.2 非目标（本期不做）

- 不做云端同步、NAS、相册在线浏览。
- 不修改照片像素/画质（不压缩、不转码，除非将来单独加「视频优化」）。
- 不做人脸识别、地点智能打标等 AI 功能。
- 不硬删除文件：所有「重复」默认先隔离到回收区，由用户后续手动清理。

### 1.3 核心原则

- **安全第一**：默认 dry-run，只有用户显式确认后才真正移动文件。
- **可回滚**：每次执行都写日志（before → after 路径映射），支持一次撤销。
- **幂等**：重复运行不会造成二次归档或重复移动。
- **不覆盖**：目标位置已有同名文件时追加计数后缀，绝不覆盖已有内容。

## 2. 技术选型

- **GUI 外壳**：Tauri 2（跨平台，macOS 优先，体积小、启动快）。
- **前端**：Vue 3 + TypeScript + Vite（现代 UI，便于做进度条、树状预览、勾选交互）。
- **核心逻辑**：Rust（`src-tauri` 内），这是本任务计算量最大的部分：
  - 目录遍历、EXIF/元数据解析、哈希、感知哈希、去重分组。
  - 用 `rayon` 做并行，用 `blake3` 做快速哈希。
- **其它关键 crate**：
  - `ignore`：快速递归遍历（无需遵循 gitignore 时可换成 `walkdir`）。
  - `kamadak-exif`：JPEG 等常见格式的 EXIF 快速读取。
  - `image` + 自实现 dHash：图片解码与感知哈希（用于相似图去重）。
  - `serde` / `serde_json`：与前端交互数据结构。
  - 外部命令：`exiftool`（HEIC/RAW/视频等通用元数据兜底）、`ffprobe`（视频拍摄时间）。

### 2.1 为什么是 Tauri + Rust，而不是 Electron + Node

- 去重（尤其相似图、相似视频）是 CPU 密集任务，Rust 性能与内存占用更优，打包产物更小。
- 可直接复用 Rust 生态里的成熟算法（如 czkawka 的相似图思路），必要时参考 [czkawka-core](https://github.com/qarmin/czkawka) /
  [fclones](https://github.com/pkolaczk/fclones) 的实现，而不是从零重写。
- 前端仍用 Vue/TS，开发体验与 Electron 一致，但不再需要打包整个 Node 运行时。

## 3. 功能需求（GUI 页面）

1. **首页 / 目录选择**
   - 添加/删除一个或多个「源目录」，选择「目标根目录」。
   - 选项：是否递归子目录、是否跟随符号链接（默认否）。
2. **扫描配置页**
   - 支持的照片/视频扩展名（见 §5），可勾选 RAW 与 HEIC 是否纳入。
   - 日期获取优先级（见 §6，可调整顺序）。
   - 去重开关与阈值：
     - 字节级去重开关（默认开）。
     - 相似图片去重开关 + 汉明距离阈值（默认开，阈值中等）。
     - 相似视频去重（默认关，标记为实验性）。
   - 命名规则：保持原文件名 / 加日期前缀（默认保持原文件名）。
3. **扫描进行页**
   - 实时进度：已扫描文件数、识别出的媒体数、重复组数、无法确定日期的文件数。
   - 可取消。
4. **预览 / 计划页（关键）**
   - 树状展示「当前路径 → 归档路径（`年/年月`）」。
   - 每条媒体项显示可放大的缩略图预览；照片尽量直接预览，RAW / 视频在无法内嵌时给出类型占位。
   - 重复文件组高亮并支持并排预览对比，标注「保留哪一个、隔离哪些」及理由；可逐项勾选要隔离的文件。
   - 支持单个勾选/取消，不勾选则不处理该文件。
   - 提供「返回修改规则 / 放弃本次会话」的安全退出，执行确认后仍可“再检查一下”取消。
   - 显示汇总：将移动 N 个文件、将隔离 M 个文件、可释放空间（估算）。
5. **执行 / 结果页**
   - 确认后执行，显示进度。
   - 完成后展示报告：成功移动数、隔离数、异常数、释放空间估算。
   - 「撤销本次执行」按钮（依据日志回滚）。

## 4. 核心处理流程

```
选择目录 → 扫描枚举文件 → 分类(照片/视频/其它) → 解析拍摄日期
        → 计算去重 key(字节哈希/感知哈希) → 构建归档计划
        → 用户预览确认 → 执行(移动+隔离) → 写日志 → 出报告
```

- 每一步结果都对前端可见；执行前必须经过预览确认。
- 扫描阶段只读，绝不写文件。

## 5. 支持的媒体类型

- **照片**：`.jpg .jpeg .png .heic .heif .webp .tif .tiff`
- **RAW**：`.dng .cr2 .cr3 .nef .arw .orf .rw2 .raf`（依赖 exiftool）
- **视频**：`.mp4 .mov .m4v .avi .mkv .3gp .webm`
- **排除**：缩略图缓存、`._*`、`.DS_Store`、`.picasa.ini`、`.thumbs` 等。

## 6. 拍摄日期解析优先级

按以下顺序取第一个可用时间，前一步失败则降级：

1. EXIF `DateTimeOriginal`（照片首选）。
2. EXIF `CreateDate` / `DateTimeDigitized`（相机写入时间）。
3. 视频元数据 `QuickTime:CreateDate` / `MediaCreateDate`（`exiftool` 或 `ffprobe` 的
   `format.tags.creation_time`）。
4. 文件系统时间：优先「创建时间」（`birthtime`，macOS 较准），其次 `mtime`。
5. 文件名内嵌日期：正则匹配 `(\d{4})[-_]?(\d{2})[-_]?(\d{2})` 等常见格式。
6. 全失败：归入「未知」分桶，在报告中列出，绝不丢弃。

> 注意：RAW/HEIC/视频优先走 `exiftool`，JPEG 可用 `kamadak-exif` 快路径。
> 时区问题：优先保留元数据原始时间，若存在偏移信息则在报告中注明，不做强制换算。

## 7. 归档目录与命名规范

```
<目标根目录>/
  2024/
    2024-05/
      原文件名.jpg
      原文件名 (1).jpg   # 冲突时追加计数，绝不覆盖
  unresolved/
    ...
  _duplicates/           # 隔离区（默认不删除，等待用户清理）
```

- 目录层级固定为「年 / 年月」，如 `2024/2024-05`。
- 默认保留原文件名；开启「日期前缀」后改为 `YYYYMMDD_HHMMSS_原文件名`。
- 冲突策略：同一目标路径已存在不同内容时，追加 ` (1)`/` (2)` 计数；若为字节完全相同则直接计为重复。

## 8. 去重策略

### 8.1 字节级重复（必做，默认开）

- 先按文件大小分组，大小相同的再算 `blake3` 哈希，组内哈希相同即为字节重复。
- 每组的保留规则：优先保留「目标路径更规整 / 文件名更短 / 更早创建」的一个，其余隔离。

### 8.2 相似图片（做，默认开）

- 用 `image` 解码后自实现 dHash（9x8 灰度相邻比较，也可换 pHash），汉明距离小于阈值视为相似。
- 同一相似组的保留规则：优先保留分辨率最高、体积最大或最清晰的一张，其余隔离。
- 阈值做成可配置，默认取「保守」档，避免误判。

### 8.3 相似视频（实验性，默认关）

- 方案：`ffmpeg` 抽帧 → 对帧做感知哈希 → 近似比较；或调用参考项目 `czkawka` 的相似视频能力。
- 后续迭代再接，本期只需留接口与开关。

### 8.4 隔离与清理

- 「重复/相似」的处理是「移动到 `_duplicates/`」，不是 `rm`。
- 提供一个「彻底删除隔离区」的二次确认入口，需两次确认。

## 9. 安全、日志与回滚

- **dry-run 默认**：扫描与计划阶段不写任何文件。
- **执行日志**：每次「应用」写入一个带时间戳的 JSON 日志，每条记录含
  `{ action, from, to, hash, reason, timestamp }`。
- **回滚**：依据日志把 `move/move_duplicate` 反向 `from ← to` 还原；无法还原（目标被占用等）时在报告里列出。
- **异常处理**：任何单文件失败不中断整体，收集到异常清单，结束后展示。
- **幂等**：再次运行应能跳过已归入正确位置的同类文件，避免重复操作。

## 10. 项目结构

```
tourism-media-organizer/
  package.json
  vite.config.ts
  tsconfig.json
  index.html
  src/                        # Vue 前端
    main.ts
    App.vue
    api/                      # 封装 invoke / event listen
    views/
      HomeView.vue            # 目录选择
      ConfigureView.vue       # 扫描配置
      ScanProgressView.vue    # 扫描进度
      PlanView.vue            # 预览/计划
      ResultView.vue          # 结果与回滚
    components/
    stores/
    styles/
  src-tauri/                  # Rust 核心
    Cargo.toml
    tauri.conf.json
    capabilities/
    icons/
    src/
      main.rs
      lib.rs
      commands.rs             # #[tauri::command]，前后端桥接
      models.rs               # serde 数据结构
      scan.rs                 # 遍历 + 分类
      date.rs                 # 日期解析与优先级
      dedup/
        mod.rs
        byte.rs               # blake3 字节级去重
        perceptual.rs         # dHash 相似图（image 0.25）
      tools.rs                # exiftool / ffprobe 探测与手动路径
      plan.rs                 # 构建计划
      apply.rs                # 执行计划
      journal.rs              # 日志写入与回滚
```

## 11. 分阶段执行任务（给执行 agent）

- **M0 脚手架**：`pnpm create tauri-app`（或等价脚手架），跑通空窗口，确认 macOS 可构建运行。
- **M1 扫描模块**：`scan.rs` 递归枚举 + 扩展名分类，能向前端返回文件清单。
- **M2 日期解析**：`date.rs` 实现 §6 优先级，覆盖 JPEG/HEIC/RAW/视频，输出到模型。
- **M3 去重**：先实现 `byte.rs` 字节级去重；再实现 `perceptual.rs` 相似图。
- **M4 计划与执行**：`plan.rs` 生成计划、`apply.rs` 执行、`journal.rs` 日志与回滚。
- **M5 GUI**：五个页面串起完整流程，含进度事件、树状预览、勾选、确认执行、结果与回滚。
- **M6 收尾**：异常处理、日志级别、单元测试与一个端到端验收脚本。

## 12. 验收标准

- [ ] dry-run 默认开启，未确认前不会改动任何文件。
- [ ] 能正确识别照片/视频/RAW，并按 §6 优先级得到拍摄日期。
- [ ] 归档结构符合 `年/年月`，冲突时追加计数、绝不覆盖。
- [ ] 字节级重复能被正确识别并只保留一个，其余进入 `_duplicates/` 隔离。
- [ ] 相似图片能在可配置阈值下识别，保守档无误删。
- [ ] 每次执行产出可读日志，且支持一次回滚。
- [ ] 重复运行幂等，不产生二次归档/重复移动。
- [ ] GUI 在 macOS 上从选择目录到查看结果全流程可用。
- [ ] 提供一个最小自动化测试：对构造的样例目录跑流程，断言移动与去重结果正确。

## 13. 参考项目

- [qarmin/czkawka](https://github.com/qarmin/czkawka)：Rust 去重工具，相似图/视频与哈希去重算法可参考。
- [pkolaczk/fclones](https://github.com/pkolaczk/fclones)：字节级重复快速查找，哈希/分组思路可参考。
- [ivandokov/phockup](https://github.com/ivandokov/phockup)：EXIF 日期归档到 `年/月` 的轻量实现。
- [tfeldmann/organize](https://github.com/tfeldmann/organize)：规则化文件整理与去重、dry-run 思路可参考。

## 14. 发布与外部工具依赖

- 打包：使用 Tauri 官方 `tauri icon` 生成 mac/windows/linux 全套图标，`bundle.active` 置为
  `true`，用 GitHub Actions + `tauri-action` 在三个系统 runner 并行出包（详见 `ARCHITECTURE.md §13`）。
- 外部命令：`exiftool`（HEIC/RAW 元数据）与 `ffprobe`（视频拍摄时间）为运行期可选依赖，
  探测与降级策略见 `ARCHITECTURE.md §12`；两者缺失时功能降级但不丢文件，
  并提供 GUI 手动指定路径的入口。
- 双平台验收：同一套代码在 macOS 与 Windows 上均需跑通「扫描 → 预览 → 执行 → 回滚」的最小
  用例；Windows 重点验证 `exiftool`/`ffprobe` 探测与长路径目录。

## 15. 当前实施状态（2026-10-31）

- M0-M6 的代码与文档已落地，包含五页 GUI、后台扫描/执行线程、计划分页与勾选、
  JSON 日志、一次回滚、隔离区双重确认删除、外部工具手动配置；计划页已加入
  Tauri asset protocol 驱动的媒体缩略图与放大预览，支持点击文件行切换勾选，并已加入重复组并排对比选择。
- Rust 单元/集成测试、`cargo clippy --all-targets -- -D warnings`、Vue 类型检查和
  Vite 生产构建均已通过。
- `src-tauri/examples/e2e_smoke.rs` 已通过完整流程：
  扫描 → 字节去重 → 计划 → 执行 → 幂等复扫 → 回滚。
- macOS 已实际启动 Tauri 窗口，并成功产出 `.app` 与 `.dmg`。
- Windows 兼容代码、长路径 manifest、PowerShell E2E 入口和三平台 CI/Release
  workflow 已提供；首次 Windows runner 实机结果需在代码推送后查看 GitHub Actions。
- 最后一层人工验收建议在真实旅游素材副本上执行：先 dry-run 抽查计划，再执行少量文件，
  检查 `_duplicates/` 与日志，最后测试一次回滚。
