# 技术栈与系统架构（实施基线）

> 本文档是可执行的架构基线：技术栈、模块划分、进程/线程模型、前后端契约。
> `PLAN.md` 定义「做什么」，本文档定义「怎么搭」。实施与评审都以本文档为准。
> 有冲突时以本文档为准，并同步更新 `PLAN.md`。

## 1. 技术栈定版

### 1.1 运行时与工具链

| 项 | 选型 | 版本基线 |
| --- | --- | --- |
| 语言（核心） | Rust | stable，edition 2021 |
| 桌面外壳 | Tauri | 2.x |
| 前端框架 | Vue 3（`<script setup>` + TS） | ^3.5 |
| 构建 | Vite | ^6 |
| 语言（前端） | TypeScript | ^5.6 |
| 路由 / 状态 | vue-router / Pinia | ^4 / ^2 |
| 前端工具库 | @vueuse/core、@tauri-apps/api | 2.x |
| 包管理 | pnpm | 任意近期版本 |
| 运行 Node（仅构建期） | Node.js | ≥ 20（LTS） |
| Cron/定时 | 不涉及 | — |

### 1.2 Rust 依赖（src-tauri/Cargo.toml 关键项）

- GUI/桥接：`tauri`（特性 `macos-private-api` 可选）、`tauri-plugin-dialog`、`serde`、`serde_json`。
- 遍历：`jwalk`（并行枚举）；备选 `walkdir`。
- 并行与哈希：`rayon`、`blake3`（开启 `rayon` 特性做多线程哈希）。
- 元数据：`kamadak-exif`（JPEG 快路径）、外部 `exiftool` / `ffprobe`（HEIC/RAW/视频兜底）。
- 感知哈希：`image` 0.25 + 自实现 dHash（避免 img_hash 绑定旧版 image 0.23）。
- 时间：`chrono`（+ `serde`），时区仅记录、不强制换算。
- 标识与路径：`uuid`（v4 作为 run_id）、`directories`（日志/日志目录定位）。
- 错误：`thiserror`、`anyhow`；日志：`tracing` + `tracing-subscriber` + `tracing-appender`。
- 测试：`tempfile`（集成测试用临时目录）。

### 1.3 前端依赖

- 生产：`vue`、`pinia`、`vue-router`、`@vueuse/core`、`@tauri-apps/api`、`@tauri-apps/plugin-dialog`。
- 开发：`vite`、`@vitejs/plugin-vue`、`typescript`、`vue-tsc`、`vitest`、`@vue/test-utils`（可选）。
- 不引入重型 UI 组件库；自建组件 + CSS 变量设计令牌，保持轻量与可控样式。

## 2. 系统分层与数据流

```
┌──────────────────────────────────────────────────────────┐
│                      前端 (Vue 3 + TS)                    │
│  Views → Stores (Pinia) → api/ (invoke + listen)          │
└───────────────▲──────────────────────────┬────────────────┘
        invoke() │ (命令请求)               │ events (进度/完成)
                 │                          │
        ┌────────┴──────────────────────────▼────────────────┐
        │              Tauri IPC 边界（必须显式契约）          │
        │  #[tauri::command] 命令  +  AppHandle::emit 事件     │
        └────────▲──────────────────────────┬────────────────┘
                 │                          │
        ┌────────┴──────────────────────────▼────────────────┐
        │                 Rust 核心（src-tauri）               │
        │  commands → services → domain modules               │
        │  扫描/去重/计划/执行/日志，后台线程 + 进度通道         │
        └─────────────────────────────────────────────────────┘
                 │ 读写
        ┌────────▼────────────────────────┐
        │  文件系统（源目录、目标目录、日志）  │
        └─────────────────────────────────┘
```

- 前端只通过 `invoke`/`listen` 与后端交互，不直接访问文件系统。
- 后端是唯一有文件写权限的一方；前端只展示与确认。

## 3. Rust 后端架构

### 3.1 模块职责

```
src/
  main.rs              # 启动、安装 tracing、注册命令与状态
  lib.rs               # tauri::Builder、状态装配、命令注册
  state.rs             # AppState：运行中的 run 会话、缓存、路径配置
  commands.rs          # 所有 #[tauri::command] 入口（薄层，转调 services）
  models.rs            # serde 数据结构（§4）
  scan.rs              # 枚举 + 分类 + 元数据读取 + 哈希（可并行）
  date.rs              # 拍摄日期解析（PLAN §6 优先级）
  tools.rs             # exiftool/ffprobe 探测、常见路径与手动配置持久化
  dedup/
    mod.rs             # 去重编排：先字节级，后相似图
    byte.rs            # 大小分组 + blake3
    perceptual.rs      # dHash 感知哈希 + 汉明距离分组
  plan.rs              # 由 inventory 生成 Plan（归档/隔离/保留决策）
  apply.rs             # 执行计划、幂等检查、冲突追加计数
  journal.rs           # 日志读写与回滚
  events.rs            # 进度事件 payload 与 emit 助手
  report.rs            # 统计汇总（释放空间估算、异常清单）
  logging.rs           # stderr + 本地滚动日志
```

### 3.2 线程模型

- Tauri 主线程负责 UI 与命令调度，**严禁在命令内做长阻塞操作**。
- 每次 `scan`/`apply` 启动一个独立后台线程运行流水线（`std::thread`）。
- 后台线程持有 `AppHandle` Clone，直接调用 `AppHandle::emit(...)` 推送进度；
  命令线程只负责校验、创建会话和立即返回。
- 取消：`AtomicBool` 标志位，流水线每批检查一次并提前退出，前端再收到 `canceled`。
- 单个 `run_id` 同时只允许一个活动流水线；新 run 用新 `uuid`。

### 3.3 会话状态（AppState）

- `HashMap<RunId, RunSession>`，`Mutex` 保护。
- `RunSession` 保存：`ScanOptions`、扫描进度、`Vec<MediaFile>`（inventory）、`Plan`、
  已应用结果引用、取消标志、日志路径。
- 会话为内存态（进程内）；日志文件落盘持久化，用于回滚与恢复。

## 4. 前后端契约（IPC）

### 4.1 Tauri 命令（invoke 名称 / 入参 / 返回）

| invoke 命令 | 入参 | 返回 |
| --- | --- | --- |
| `scan` | `ScanOptions` | `RunStarted { run_id }` |
| `cancel_scan` | `run_id: String` | `()` |
| `get_scan_status` | `run_id: String` | `ScanStatus` |
| `get_plan_summary` | `run_id: String` | `PlanSummary` |
| `get_plan_page` | `run_id, cursor: String?, page_size: u32` | `PlanPage` |
| `set_item_selected` | `run_id, item_id, selected: bool` | `()` |
| `set_all_selected` | `run_id, selected: bool` | `()` |
| `apply` | `run_id: String` | `RunStarted { run_id }`（复用同一 run_id） |
| `get_apply_result` | `run_id: String` | `ApplyResult` |
| `rollback` | `run_id: String` | `RollbackResult` |
| `delete_quarantine` | `run_id, confirmation: "DELETE_QUARANTINE"` | `QuarantineDeleteResult` |
| `get_tool_status` | — | `ToolStatus` |
| `set_tool_paths` | `exiftool_path?, ffprobe_path?` | `ToolStatus` |

### 4.2 事件（emit 名称 / payload）

| 事件 | payload | 说明 |
| --- | --- | --- |
| `scan://progress` | `ScanProgress` | 扫描阶段实时进度 |
| `scan://done` | `ScanDone` | 库存构建完成（含 stats） |
| `apply://progress` | `ApplyProgress` | 应用阶段进度 |
| `apply://done` | `ApplyResult` | 应用完成或失败汇总 |

### 4.3 数据模型（serde / 对应 TS interface）

```rust
#[derive(Serialize, Deserialize)]
struct ScanOptions {
    source_dirs: Vec<PathBuf>,
    target_dir: PathBuf,
    recursive: bool,            // 默认 true
    follow_symlinks: bool,      // 默认 false
    include_raw: bool,          // 默认 true
    include_heic: bool,         // 默认 true
    date_priority: Vec<DateSource>,
    dedup: DedupOptions,
    naming: NamingOptions,
}

enum DateSource {
    ExifDateTimeOriginal,
    ExifCreateDate,
    VideoMetadata,
    FileBirthTime,
    FileMtime,
    Filename,
}

struct DedupOptions {
    byte: bool,                       // 默认 true
    similar_images: Option<SimilarityOptions>, // 默认 Some
    similar_videos: Option<SimilarityOptions>, // 默认 None（实验）
}

struct SimilarityOptions {
    hamming_threshold: u32,   // 默认保守档，例：8
}

struct NamingOptions {
    prefix_date: bool,        // 默认 false，保持原文件名
}
```

```rust
enum MediaKind { Photo, Raw, Video }

struct MediaFile {
    id: String,                // 稳定标识（路径哈希）
    source_path: PathBuf,
    kind: MediaKind,
    size_bytes: u64,
    ext: String,               // 不含点，小写
    date: Option<DateTime<Utc>>,
    date_source: Option<DateSource>,
    resolved: bool,            // false 表示日期未知
    byte_hash: Option<String>, // blake3 hex
    image_hash: Option<String>,// 感知哈希 hex（仅照片且开启相似图）
}
```

```rust
struct ScanProgress {
    run_id: String,
    phase: ScanPhase,       // Enumerating|ReadingMetadata|Hashing|Dedup|Done
    total_files: u64,
    processed_files: u64,
    media_found: u64,
    duplicate_groups: u64,
    similar_groups: u64,
    unresolved: u64,
    message: Option<String>,
}

struct ScanDone { run_id: String, stats: ScanStats }
struct ScanStatus { phase: ScanPhase, done: bool, canceled: bool, stats: Option<ScanStats> }
```

```rust
enum PlanAction { Archive, Isolate, Keep, Unresolved }

struct PlanItem {
    id: String,
    source_path: PathBuf,
    target_path: Option<PathBuf>, // Archive/Isolate/Unresolved 时非空；Keep 为空
    action: PlanAction,
    reason: Option<String>,       // byte_duplicate / similar_image / conflict / ...
    group_id: Option<String>,     // 去重组标识
    kind: MediaKind,
    size_bytes: u64,
    date: Option<DateTime<Utc>>,
    selected: bool,               // 默认 true
}

struct PlanSummary {
    run_id: String,
    items_total: u64,
    items_selected: u64,
    archive_count: u64,
    isolate_count: u64,
    unresolved_count: u64,
    keep_count: u64,
    freed_est_bytes: u64,      // 被隔离项体积之和
    duplicate_groups: u64,
    similar_groups: u64,
}

struct PlanPage { items: Vec<PlanItem>, next_cursor: Option<String> }

struct ApplyProgress { run_id: String, done: u64, total: u64, phase: String }
struct ApplyResult {
    run_id: String,
    archived: u64,
    isolated: u64,
    unresolved: u64,
    freed_est_bytes: u64,
    failed: Vec<Failure>,
    journal_path: PathBuf,
    canceled: bool,
}
struct RollbackResult { run_id: String, restored: u64, failed: Vec<Failure>, journal_path: PathBuf }
struct Failure { path: PathBuf, code: String, message: String }
struct QuarantineDeleteResult { run_id: String, deleted_files: u64, freed_bytes: u64, quarantine_path: PathBuf }
```

## 5. 关键算法与决策规则

### 5.1 扫描阶段

1. `jwalk` 并行枚举，过滤隐藏文件与排除项（PLAN §5）。
2. 按扩展名分类为 Photo / Raw / Video / Other（Other 忽略）。
3. 并发读取日期：JPEG 用 `kamadak-exif`；HEIC/RAW/视频用 `exiftool`，视频再补 `ffprobe`；
   降级顺序严格按 `date_priority`。
4. 需要去重时：先读大小，`dedup.byte` 开启则对同大小组算 `blake3`；
   `dedup.similar_images` 开启且为照片时用 `image` 解码后算 dHash 感知哈希。
5. 产出 `MediaFile` 清单，缓存于会话，发 `scan://done`。

### 5.2 去重分组（dedup/）

- **字节级**：`size → blake3`。同 hash 为一组，保留一个，其余 `Isolate`。
- **相似图**：感知哈希两两/近邻分组（组内汉明距离 <= 阈值）。保留「体积最大、
  日期最早、路径更规整」的一张，其余 `Isolate`。
- 每个重复组的保留项仍进入 `Archive`，不会遗留在散乱源目录；其余副本进入
  `Isolate`。`Keep` 只用于「源路径已经等于目标路径」的幂等场景。

### 5.3 计划生成（plan.rs）

- 对每个 `MediaFile` 计算 `target_path = target_dir/YYYY/YYYY-MM/原名`。
- 已存在同名：
  - 字节完全相同 → 记入 `Isolate`（重复）。
  - 内容不同 → 追加 ` (1)`/` (2)` 计数。
- 日期 `resolved==false` → `Unresolved`，归入 `unresolved/`，不参与去重隔离。
- 源路径已经等于目标路径 → `Keep`，保证重复扫描幂等。
- 生成 `PlanItem` 与 `PlanSummary`，分页返回前端。

### 5.4 执行与回滚（apply.rs / journal.rs）

- 仅处理 `selected==true` 的项；目标目录不存在则创建。
- 先 `move` 普通归档，再 `move` 隔离项到 `_duplicates/`（保持相对内层结构以减少重名）。
- 每步写 `JournalEntry`；全部完成后落盘 `journal/<run_id>.json`。
- 为避免 Unix `rename` 的覆盖语义，执行层同卷使用「硬链接 → 删除源名」，
  跨卷或文件系统不支持硬链接时降级为「`create_new` 复制 → `sync_all` → 删除源」；
  两条路径都不会覆盖已有目标。任何单文件失败只进入报告，不中断整批。
- 回滚按日志逆序执行 `from ← to`，失败项进 `failed`。
- 彻底删除 `_duplicates/` 是单独命令，要求前端两次确认并携带固定确认串；删除后
  禁止回滚，避免产生“部分恢复”的误导结果。

## 6. 错误与日志

- 命令边界当前返回用户可读的 `Result<T, String>`；批处理中的单文件错误统一收集为
  `Failure { path, code, message }`，不会吞掉或中断其它文件。
- 前端在表单、扫描页、执行页分别展示错误；原始底层错误仅作诊断补充。
- 后端 `tracing` 同时输出到 stderr 与
  `<app-data>/logs/tourism-media-YYYY-MM-DD.log`（`directories` 定位应用数据目录）。
- 执行日志位于 `<app-data>/journals/<run_id>.json`，JSON 写入使用临时文件替换，
  兼容 Windows 不允许 rename 覆盖已存在文件的语义。

## 7. 前端架构

```
src/
  main.ts                 # createApp + pinia + router
  App.vue                 # 顶层布局 + 路由出口
  api/
    commands.ts           # 封装 invoke(...) 类型化
    events.ts             # 封装 listen(...)，转发到 store
    types.ts              # §4 数据模型 TS 镜像
  stores/
    scan.store.ts         # 扫描状态机、进度、inventory/plan 缓存、选择集
  router.ts               # Home → Configure → Scan → Plan → Result
  views/
    HomeView.vue
    ConfigureView.vue
    ScanProgressView.vue
    PlanView.vue
    ResultView.vue
  components/
    FileTreeItem.vue      # 树状项 + 勾选
    DuplicateGroup.vue    # 重复组高亮
    StatsBar.vue          # 汇总条
    ProgressBar.vue
  styles/
    tokens.css            # CSS 变量：色板/字体/间距/层级
    base.css
```

- 导航为单向流：`Home(选目录) → Configure(选项) → Scan(进度) → Plan(预览) → Result(结果)`。
- `scan.store.ts` 是唯一数据源：run_id、进度、计划摘要、计划分页缓存、勾选状态。
- 事件到达后写 store，组件只读 store；勾选操作走 `set_item_selected` 命令后回写 store。

## 8. 状态机

```
Idle
  └─ scan → Scanning (可 cancel → Idle)
       └─ scan://done → Scanned
            └─ get_plan → Planned
                 └─ apply → Applying (可失败 → Planned/Failed)
                      └─ apply://done → Applied
                           ├─ rollback → RolledBack
                           └─ delete_quarantine → Applied（禁止再回滚）
```

## 9. 构建与打包

- 开发：`pnpm tauri dev`（Vite 热更 + Rust 重编译）。
- 类型检查：`pnpm tauri build` 前 `vue-tsc --noEmit` 与 `cargo clippy`。
- 打包：`pnpm tauri build` 产出 macOS `.app` / `.dmg`。
- 版本管理：`src-tauri/Cargo.toml` 的 `version` 与 `package.json` 的 `version` 保持一致。

## 10. 测试策略

- **Rust 单元测试**：`date.rs` 日期优先级、`dedup/byte.rs` 分组、`dedup/perceptual.rs` 汉明距离阈值。
- **Rust 集成测试**：`tempfile` 构造样例目录，断言归档目录与 `_duplicates/` 内容、日志可回滚。
- **前端单元**：`scan.store.ts` 状态迁移、调 API 的 mock。
- **端到端（验收）**：`src-tauri/examples/e2e_smoke.rs` 构造「字节重复 + 不同日期 +
  无元数据」样例，跑扫描、去重、计划、执行、幂等复扫和回滚；
  `scripts/e2e-smoke.sh` / `.ps1` 为跨平台入口。

## 11. 实施顺序建议（对应 PLAN §11）

- M0 脚手架 → M1 扫描 → M2 日期 → M3 去重 → M4 计划/执行/回滚 → M5 GUI → M6 测试收尾。
- 每阶段完成即验证对应命令与事件契约，避免前端/后端联调滞后。

## 12. 外部命令探测与降级（exiftool / ffprobe）

- 启动时（或首次扫描前）探测 `exiftool`、`ffprobe` 是否在 `PATH` 可用：
  各执行一次 `-ver` / `-version`，结果缓存在 `AppState`，不重复探测。
- 能力矩阵（`date.rs` 按此选择解析器）：
  - JPEG：`kamadak-exif` 内置解析，失败再尝试 `exiftool`。
  - PNG / WebP / TIFF：`kamadak-exif` 或 `exiftool`。
  - HEIC / HEIF：优先 `exiftool`（`kamadak-exif` 对 HEIF 容器支持有限）。
  - RAW（CR2/CR3/NEF/ARW 等）：完全依赖 `exiftool`。
  - 视频（MP4/MOV/…）：优先 `ffprobe`（`format.tags.creation_time`）；缺失则
    `exiftool`（`QuickTime:CreateDate` / `MediaCreateDate`）。
- 降级顺序严格遵循 `PLAN.md §6`；两者都缺失时，仍把「无法解析日期的文件」完整列出到报告，
  并给出安装提示，绝不静默跳过或误判为可删除。
- Windows 特殊处理：
  - 除 `PATH` 外，检查常见安装路径（scoop/choco shims、`C:\Program Files\...`）。
  - 提供 GUI 入口「手动指定 exiftool/ffprobe 路径」，写入本地配置，后续扫描复用。
  - 已实现 `tools.json` 持久化、规则页手动选择，以及 Windows app manifest 的
    `longPathAware=true`。

## 13. 打包与多平台发布

- 开发验证用 `pnpm tauri dev`；发布用 `pnpm tauri build`。
- 图标：以一张 1024×1024 PNG 为源运行 `pnpm tauri icon <src.png>`，输出到
  `src-tauri/icons/`，覆盖 macOS `.icns`、Windows `.ico`、Linux `.png` 全套；随后把
  `tauri.conf.json` 的 `bundle.active` 改为 `true`，`bundle.icon` 使用生成的全套文件名。
- 当前仓库已提供 `app-icon.svg`、完整图标集、`bundle.active=true`，并在 macOS
  实际产出 `.app` 与 `.dmg`。
- 平台打包原则：Tauri 建议在目标系统构建。用 GitHub Actions + `tauri-action` 在
  `windows-latest` / `macos-latest` / `ubuntu-latest` 三个 runner 上并行出包，
  产物（`.exe`/`.msi`、`.app`/`.dmg`、`.AppImage`/`.deb`）作为 release 附件。
