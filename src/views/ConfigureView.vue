<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useRouter } from "vue-router";
import { open } from "@tauri-apps/plugin-dialog";
import type { DateSource } from "../api/types";
import { getToolStatus, setToolPaths } from "../api/commands";
import { useScanStore } from "../stores/scan.store";

const router = useRouter();
const store = useScanStore();
const formError = ref<string | null>(null);
const exiftoolPath = ref("");
const ffprobePath = ref("");
const toolReady = ref({ exiftool: false, ffprobe: false });
const toolError = ref<string | null>(null);
const toolSaved = ref(false);

const dateLabels: Record<DateSource, string> = {
  exif_date_time_original: "EXIF 原始拍摄时间",
  exif_create_date: "EXIF 创建 / 数字化时间",
  video_metadata: "视频元数据",
  file_birth_time: "文件创建时间",
  file_mtime: "文件修改时间",
  filename: "文件名中的日期",
};

const similarImagesEnabled = computed({
  get: () => store.options.dedup.similar_images !== null,
  set: (enabled: boolean) => {
    store.options.dedup.similar_images = enabled
      ? { hamming_threshold: 8 }
      : null;
  },
});

function moveDate(index: number, direction: -1 | 1) {
  const target = index + direction;
  const list = [...store.options.date_priority];
  if (target < 0 || target >= list.length) return;
  [list[index], list[target]] = [list[target], list[index]];
  store.options.date_priority = list;
}

onMounted(async () => {
  try {
    const status = await getToolStatus();
    exiftoolPath.value = status.exiftool_path ?? "";
    ffprobePath.value = status.ffprobe_path ?? "";
    toolReady.value = {
      exiftool: Boolean(status.exiftool_path),
      ffprobe: Boolean(status.ffprobe_path),
    };
  } catch (error) {
    toolError.value = String(error);
  }
});

async function pickTool(kind: "exiftool" | "ffprobe") {
  try {
    const picked = await open({ directory: false, multiple: false });
    if (typeof picked === "string") {
      if (kind === "exiftool") exiftoolPath.value = picked;
      else ffprobePath.value = picked;
      toolSaved.value = false;
    }
  } catch (error) {
    toolError.value = String(error);
  }
}

async function saveTools() {
  toolError.value = null;
  toolSaved.value = false;
  try {
    const status = await setToolPaths(
      exiftoolPath.value.trim() || null,
      ffprobePath.value.trim() || null,
    );
    exiftoolPath.value = status.exiftool_path ?? "";
    ffprobePath.value = status.ffprobe_path ?? "";
    toolReady.value = {
      exiftool: Boolean(status.exiftool_path),
      ffprobe: Boolean(status.ffprobe_path),
    };
    toolSaved.value = true;
  } catch (error) {
    toolError.value = String(error);
  }
}

async function start() {
  formError.value = null;
  try {
    await store.start();
    await router.push("/scan");
  } catch (error) {
    formError.value = String(error);
  }
}
</script>

<template>
  <section class="configure">
    <header class="section-head">
      <div>
        <p class="eyebrow">TRAVEL ARCHIVE / 02</p>
        <h2>定义怎样判断时间与重复。</h2>
      </div>
      <button class="btn btn--ghost" @click="router.push('/')">返回目录</button>
    </header>

    <div class="configure__grid">
      <article class="card setting-card">
        <h3>扫描范围</h3>
        <p class="setting-card__note">扫描阶段只读，目标目录不会在确认前被创建。</p>
        <label class="switch-row">
          <span>递归扫描子目录</span>
          <input v-model="store.options.recursive" type="checkbox" />
        </label>
        <label class="switch-row">
          <span>跟随符号链接</span>
          <input v-model="store.options.follow_symlinks" type="checkbox" />
        </label>
        <label class="switch-row">
          <span>包含 RAW 文件</span>
          <input v-model="store.options.include_raw" type="checkbox" />
        </label>
        <label class="switch-row">
          <span>包含 HEIC / HEIF</span>
          <input v-model="store.options.include_heic" type="checkbox" />
        </label>
      </article>

      <article class="card setting-card">
        <h3>重复识别</h3>
        <p class="setting-card__note">重复项不会删除，只会移动到 `_duplicates/`。</p>
        <label class="switch-row">
          <span>字节级完全相同</span>
          <input v-model="store.options.dedup.byte" type="checkbox" />
        </label>
        <label class="switch-row">
          <span>相似图片（dHash）</span>
          <input v-model="similarImagesEnabled" type="checkbox" />
        </label>
        <div
          class="threshold"
          :class="{ 'threshold--disabled': !similarImagesEnabled }"
        >
          <div>
            <span>汉明距离阈值</span>
            <strong>
              {{ store.options.dedup.similar_images?.hamming_threshold ?? "—" }}
            </strong>
          </div>
          <input
            v-model.number="store.options.dedup.similar_images!.hamming_threshold"
            type="range"
            min="0"
            max="16"
            :disabled="!similarImagesEnabled"
          />
          <small>默认 8 较保守；数值越高识别越激进。</small>
        </div>
        <div class="experimental">
          <input type="checkbox" disabled />
          <span>相似视频识别（实验性，后续版本）</span>
        </div>
      </article>

      <article class="card setting-card setting-card--wide tools-card">
        <div class="tools-card__head">
          <div>
            <h3>外部元数据工具</h3>
            <p class="setting-card__note">
              RAW、HEIC 与视频拍摄时间依赖 exiftool / ffprobe；缺失时仍可运行，但会降级到文件时间与文件名。
            </p>
          </div>
          <button class="btn btn--ghost" @click="saveTools">保存并检测</button>
        </div>
        <div class="tool-row">
          <span class="tool-state" :class="{ ready: toolReady.exiftool }">
            {{ toolReady.exiftool ? "已发现" : "未发现" }}
          </span>
          <div>
            <strong>exiftool</strong>
            <input v-model="exiftoolPath" placeholder="自动探测，或手动指定可执行文件" />
          </div>
          <button class="btn btn--ghost" @click="pickTool('exiftool')">选择</button>
        </div>
        <div class="tool-row">
          <span class="tool-state" :class="{ ready: toolReady.ffprobe }">
            {{ toolReady.ffprobe ? "已发现" : "未发现" }}
          </span>
          <div>
            <strong>ffprobe</strong>
            <input v-model="ffprobePath" placeholder="自动探测，或手动指定可执行文件" />
          </div>
          <button class="btn btn--ghost" @click="pickTool('ffprobe')">选择</button>
        </div>
        <p v-if="toolError" class="error">{{ toolError }}</p>
        <p v-else-if="toolSaved" class="tool-saved">工具路径已保存，将用于下一次扫描。</p>
      </article>

      <article class="card setting-card setting-card--wide">
        <h3>日期优先级</h3>
        <p class="setting-card__note">
          从上到下尝试，第一个有效时间即为归档依据。全部失败会放入 `unresolved/`。
        </p>
        <ol class="priority-list">
          <li
            v-for="(source, index) in store.options.date_priority"
            :key="source"
          >
            <span class="priority-list__index">{{ index + 1 }}</span>
            <span class="priority-list__label">{{ dateLabels[source] }}</span>
            <div class="priority-list__actions">
              <button :disabled="index === 0" title="上移" @click="moveDate(index, -1)">
                ↑
              </button>
              <button
                :disabled="index === store.options.date_priority.length - 1"
                title="下移"
                @click="moveDate(index, 1)"
              >
                ↓
              </button>
            </div>
          </li>
        </ol>
      </article>

      <article class="card setting-card setting-card--wide naming-card">
        <div>
          <h3>归档命名</h3>
          <p class="setting-card__note">
            目录固定为 `年 / 年月`。文件名默认保持不变。
          </p>
        </div>
        <label class="name-choice">
          <input
            v-model="store.options.naming.prefix_date"
            type="checkbox"
          />
          <span>
            在文件名前加日期
            <code>YYYYMMDD_HHMMSS_</code>
          </span>
        </label>
      </article>
    </div>

    <footer class="configure__footer">
      <div>
        <span class="footer-label">输出位置</span>
        <code>{{ store.options.target_dir }}</code>
      </div>
      <p v-if="formError" class="error">{{ formError }}</p>
      <button class="btn btn--large" @click="start">开始只读扫描</button>
    </footer>
  </section>
</template>

<style scoped>
.configure {
  display: grid;
  gap: 20px;
}

.section-head {
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
  gap: 20px;
  padding: 10px 2px 4px;
}

.eyebrow {
  margin: 0 0 10px;
  color: var(--accent);
  font-family: var(--font-mono);
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.12em;
}

.section-head h2 {
  margin: 0;
  font-family: var(--font-display);
  font-size: clamp(28px, 4vw, 44px);
  letter-spacing: -0.04em;
}

.configure__grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 16px;
}

.setting-card h3 {
  margin: 0;
  font-size: 16px;
}

.setting-card__note {
  margin: 7px 0 16px;
  color: var(--ink-soft);
  font-size: 11px;
  line-height: 1.6;
}

.setting-card--wide {
  grid-column: 1 / -1;
}

.switch-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 12px 0;
  border-top: 1px solid var(--line);
  font-size: 13px;
}

.switch-row input,
.name-choice input {
  width: 18px;
  height: 18px;
  accent-color: var(--accent);
}

.threshold {
  margin-top: 12px;
  padding: 14px;
  border-radius: 10px;
  background: var(--bg-soft);
}

.threshold--disabled {
  opacity: 0.45;
}

.threshold > div {
  display: flex;
  justify-content: space-between;
  font-size: 12px;
}

.threshold input {
  width: 100%;
  margin: 12px 0 6px;
  accent-color: var(--accent);
}

.threshold small {
  color: var(--ink-soft);
  font-size: 10px;
}

.experimental {
  display: flex;
  gap: 8px;
  margin-top: 15px;
  color: var(--ink-soft);
  font-size: 11px;
}

.tools-card {
  display: grid;
  gap: 10px;
}

.tools-card__head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 18px;
}

.tool-row {
  display: grid;
  grid-template-columns: 64px minmax(0, 1fr) auto;
  align-items: center;
  gap: 12px;
  padding: 11px 0;
  border-top: 1px solid var(--line);
}

.tool-row strong {
  display: block;
  margin-bottom: 5px;
  font-family: var(--font-mono);
  font-size: 10px;
}

.tool-row input {
  width: 100%;
  padding: 8px 10px;
  border: 1px solid var(--line);
  border-radius: 8px;
  background: var(--bg-soft);
  color: var(--ink);
  font-family: var(--font-mono);
  font-size: 10px;
}

.tool-state {
  padding: 4px 7px;
  border-radius: 999px;
  background: var(--warm-soft);
  color: var(--warm);
  font-size: 9px;
  font-weight: 700;
  text-align: center;
}

.tool-state.ready {
  background: var(--accent-soft);
  color: var(--accent);
}

.tool-saved {
  margin: 0;
  color: var(--accent);
  font-size: 10px;
}

.priority-list {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 8px;
  margin: 0;
  padding: 0;
  list-style: none;
}

.priority-list li {
  display: grid;
  grid-template-columns: 24px minmax(0, 1fr) auto;
  align-items: center;
  gap: 9px;
  padding: 11px 12px;
  border: 1px solid var(--line);
  border-radius: 10px;
}

.priority-list__index {
  display: grid;
  width: 24px;
  height: 24px;
  place-items: center;
  border-radius: 50%;
  background: var(--accent-soft);
  color: var(--accent);
  font-family: var(--font-mono);
  font-size: 10px;
  font-weight: 700;
}

.priority-list__label {
  font-size: 11px;
}

.priority-list__actions {
  display: flex;
  gap: 3px;
}

.priority-list__actions button {
  width: 24px;
  height: 24px;
  border: 1px solid var(--line);
  border-radius: 6px;
  background: var(--bg-panel);
  color: var(--ink);
  cursor: pointer;
}

.priority-list__actions button:disabled {
  opacity: 0.25;
  cursor: default;
}

.naming-card {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 24px;
}

.name-choice {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 12px 14px;
  border-radius: 10px;
  background: var(--bg-soft);
  font-size: 12px;
}

.name-choice code {
  color: var(--accent);
  font-family: var(--font-mono);
}

.configure__footer {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 20px;
  padding: 16px 0;
}

.configure__footer > div {
  min-width: 0;
  margin-right: auto;
}

.footer-label {
  display: block;
  margin-bottom: 4px;
  color: var(--ink-soft);
  font-size: 10px;
}

.configure__footer code {
  display: block;
  overflow: hidden;
  color: var(--ink-soft);
  font-family: var(--font-mono);
  font-size: 10px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

@media (max-width: 900px) {
  .priority-list {
    grid-template-columns: repeat(2, 1fr);
  }
}

@media (max-width: 760px) {
  .configure__grid,
  .priority-list {
    grid-template-columns: 1fr;
  }

  .section-head,
  .naming-card,
  .configure__footer,
  .tools-card__head {
    align-items: stretch;
    flex-direction: column;
  }

  .tool-row {
    grid-template-columns: 1fr;
  }
}
</style>
