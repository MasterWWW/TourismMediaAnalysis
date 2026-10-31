# 目录结构与模块职责

> 当前仓库已经不是初始脚手架：M0-M6 功能均已落地。
> `PLAN.md` 定义产品验收，`ARCHITECTURE.md` 定义技术契约，本文件用于快速定位代码。

```text
TourismMediaAnalysis/
├── PLAN.md                         # 需求、验收标准与当前实施状态
├── ARCHITECTURE.md                 # 技术栈、IPC、算法、线程与打包基线
├── README.md                       # 使用、开发、测试、打包说明
├── app-icon.svg                    # 1024x1024 源图标
├── package.json                    # 前端与跨平台命令入口
├── pnpm-lock.yaml
├── pnpm-workspace.yaml
├── vite.config.ts
├── tsconfig.json
├── tsconfig.node.json
├── index.html
├── .github/
│   └── workflows/
│       ├── ci.yml                  # macOS / Windows / Ubuntu 验证
│       └── release.yml             # tag 发布三平台安装包
├── scripts/
│   ├── e2e-smoke.sh                # macOS/Linux E2E 入口
│   └── e2e-smoke.ps1               # Windows PowerShell E2E 入口
├── src/                            # Vue 3 + TypeScript
│   ├── main.ts                     # createApp + Pinia + Router
│   ├── App.vue                     # 顶部流程指示与页面壳
│   ├── router.ts                   # Hash 路由，兼容 Tauri 自定义协议
│   ├── api/
│   │   ├── types.ts                # Rust serde 契约的 TS 镜像
│   │   ├── commands.ts             # 所有 invoke 封装
│   │   └── events.ts               # scan/apply 事件订阅
│   ├── stores/
│   │   └── scan.store.ts           # 五步状态机、计划选择、执行与回滚
│   ├── views/
│   │   ├── HomeView.vue            # 源/目标目录选择
│   │   ├── ConfigureView.vue       # 扫描、去重、日期、命名、外部工具
│   │   ├── ScanProgressView.vue    # 扫描进度与取消
│   │   ├── PlanView.vue            # 年/月树、重复组、逐项勾选、二次执行确认
│   │   └── ResultView.vue          # 执行报告、异常、回滚、隔离区删除
│   ├── components/
│   │   ├── FileTreeItem.vue        # 单文件计划项
│   │   ├── DuplicateGroup.vue      # 重复组保留/隔离说明
│   │   ├── StatsBar.vue            # 汇总指标
│   │   └── ProgressBar.vue         # 扫描/执行进度
│   └── styles/
│       ├── tokens.css              # 色板、字体、间距
│       └── base.css                # 基础布局与控件
├── src-tauri/                      # Rust 核心
│   ├── Cargo.toml
│   ├── Cargo.lock
│   ├── build.rs                    # Tauri build + Windows manifest
│   ├── windows-app-manifest.xml    # asInvoker + longPathAware
│   ├── tauri.conf.json             # 窗口、打包、图标
│   ├── capabilities/default.json   # 主窗口 dialog 权限
│   ├── icons/                      # macOS/Windows/Linux 全套图标
│   ├── examples/
│   │   └── e2e_smoke.rs            # 后端完整流程验收
│   └── src/
│       ├── main.rs                 # 二进制入口
│       ├── lib.rs                  # Tauri Builder、模块与命令注册
│       ├── models.rs               # serde 数据结构
│       ├── state.rs                # AppState / RunSession
│       ├── commands.rs             # 薄 IPC 命令层
│       ├── scan.rs                 # 枚举、分类、排除、库存构建
│       ├── date.rs                 # 日期优先级与 EXIF/外部命令解析
│       ├── tools.rs                # exiftool/ffprobe 探测与持久化
│       ├── dedup/
│       │   ├── mod.rs              # 去重编排
│       │   ├── byte.rs             # size + blake3
│       │   └── perceptual.rs       # image 0.25 + dHash
│       ├── plan.rs                 # 归档/隔离/Keep/冲突计划
│       ├── apply.rs                # 不覆盖移动、跨卷复制降级
│       ├── journal.rs              # JSON 日志与逆序回滚
│       ├── report.rs               # 汇总与隔离区删除
│       ├── events.rs               # 事件名常量
│       └── logging.rs              # stderr + 滚动文件日志
├── dist/                            # Vite 生成，不提交
└── src-tauri/target/                # Cargo 生成，不提交
```

## 关键模块边界

- `commands.rs` 只做参数校验、会话创建、线程启动；不包含扫描/计划算法。
- `scan.rs` / `dedup/` 只读源文件，产出库存和分组。
- `plan.rs` 只读并按目标文件系统现状解决冲突，产出完整、可预览计划。
- `apply.rs` 只执行 `selected=true` 的项；`journal.rs` 是唯一回滚来源。
- 前端不直接访问文件系统，所有写入都必须经过上述 Rust 模块。

## 本地产物

- 回滚日志：`<app-data>/journals/<run_id>.json`
- 应用日志：`<app-data>/logs/tourism-media-YYYY-MM-DD.log`
- 工具配置：`<app-config>/tools.json`
- 隔离区：`<target>/_duplicates/`
- 未知日期：`<target>/unresolved/`

具体路径由 `directories` 按操作系统约定生成，执行结果页会展示本次日志路径。
