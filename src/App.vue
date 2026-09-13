<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch, type Component } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { openPath, openUrl } from '@tauri-apps/plugin-opener';
import {
  AlertTriangle,
  ArrowLeft,
  Clock3,
  ExternalLink,
  Moon,
  Minus,
  Power,
  RefreshCw,
  Settings,
  X,
} from 'lucide-vue-next';

type Action = 'shutdown' | 'force' | 'sleep';
type TrayQuickSchedule = { action: Action; minutes: number };
type Page = 'timer' | 'history' | 'settings' | 'about';
type Schedule = { action: Action; target_time: string };
type Settings = {
  reminders: { ten: boolean; one: boolean; thirty: boolean };
  startup: boolean;
  silent: boolean;
  exitToTray: boolean;
};
type FeedbackTone = 'success' | 'error' | 'warning' | 'info';
type Feedback = { id: number; message: string; tone: FeedbackTone };
const repositoryUrl = 'https://github.com/Zverly/NightOwlTimer';

const page = ref<Page>('timer');
const action = ref<Action>('shutdown');
const duration = ref(60);
const custom = ref('');
const activeUntil = ref<Date | null>(null);
const pending = ref(false);
const toast = ref<Feedback | null>(null);
const appVersion = __APP_VERSION__;
const history = ref<Array<{ time: string; action: string; detail: string }>>([]);
const historyRefreshing = ref(false);
const repositoryOpening = ref(false);
const updateChecking = ref(false);
const settings = ref<Settings>({
  reminders: { ten: true, one: true, thirty: true },
  startup: false,
  silent: false,
  exitToTray: true,
});
const savedSettings = ref('');
const diagnostics = ref({
  version: '-',
  platform: '-',
  history_count: 0,
  data_directory: '-',
  data_file: '-',
});
const now = ref(Date.now());
const timer = window.setInterval(() => (now.value = Date.now()), 1000);
let unlistenCancel: (() => void) | undefined;
let unlistenAdjust: (() => void) | undefined;
let unlistenSettings: (() => void) | undefined;
let unlistenQuickSchedule: (() => void) | undefined;
let reminderTimers: number[] = [];

const actions: Array<{
  id: Action;
  label: string;
  icon: Component;
  description: string;
}> = [
  {
    id: 'shutdown',
    label: '正常关机',
    icon: Power,
    description: '保存工作后安全关闭电脑',
  },
  {
    id: 'force',
    label: '强制关机',
    icon: AlertTriangle,
    description: '立即终止应用并关闭电脑',
  },
  { id: 'sleep', label: '睡眠', icon: Moon, description: '进入低功耗睡眠状态' },
];
const actionMeta = computed(() => actions.find((item) => item.id === action.value) ?? actions[0]);
const remaining = computed(() =>
  activeUntil.value ? Math.max(0, activeUntil.value.getTime() - now.value) : 0,
);
const countdown = computed(() => {
  const seconds = Math.floor(remaining.value / 1000);
  return `${String(Math.floor(seconds / 3600)).padStart(2, '0')}:${String(Math.floor(seconds / 60) % 60).padStart(2, '0')}:${String(seconds % 60).padStart(2, '0')}`;
});
const targetText = computed(
  () =>
    activeUntil.value?.toLocaleString('zh-CN', {
      hour: '2-digit',
      minute: '2-digit',
    }) ?? '-',
);
const customTargetText = computed(() =>
  custom.value
    ? new Date(custom.value).toLocaleString('zh-CN', {
        month: 'long',
        day: 'numeric',
        hour: '2-digit',
        minute: '2-digit',
      })
    : '选择一个执行时刻',
);
const presetTargetText = computed(() =>
  new Date(now.value + duration.value * 60_000).toLocaleString('zh-CN', {
    hour: '2-digit',
    minute: '2-digit',
  }),
);
const displayedDateTime = computed(
  () => custom.value || formatDate(new Date(now.value + duration.value * 60_000)),
);
const settingsDirty = computed(() => JSON.stringify(settings.value) !== savedSettings.value);
const visibleHistory = computed(() => history.value.slice(0, 6));

let feedbackId = 0;
function notify(message: string, tone: FeedbackTone = 'info') {
  const id = ++feedbackId;
  toast.value = { id, message, tone };
  window.setTimeout(() => {
    if (toast.value?.id === id) toast.value = null;
  }, 2600);
}
async function refreshHistory() {
  if (historyRefreshing.value) return;
  historyRefreshing.value = true;
  try {
    history.value = await invoke('get_history');
  } catch {
    notify('历史记录刷新失败', 'error');
  } finally {
    historyRefreshing.value = false;
  }
}
function settingSnapshot() {
  return JSON.stringify(settings.value);
}
function clearReminderTimers() {
  reminderTimers.forEach(window.clearTimeout);
  reminderTimers = [];
}
function armReminders() {
  clearReminderTimers();
  if (!activeUntil.value) return;
  const points = [
    { seconds: 600, enabled: settings.value.reminders.ten, label: '10 分钟' },
    { seconds: 60, enabled: settings.value.reminders.one, label: '1 分钟' },
    { seconds: 30, enabled: settings.value.reminders.thirty, label: '30 秒' },
  ];
  points.forEach((point) => {
    const wait = activeUntil.value!.getTime() - Date.now() - point.seconds * 1000;
    if (point.enabled && wait > 0)
      reminderTimers.push(
        window.setTimeout(() => notify(`${point.label}后将执行${actionMeta.value.label}`), wait),
      );
  });
}
async function schedule(minutes = duration.value) {
  if (pending.value) return;
  if (action.value === 'force' && !window.confirm('强制关机会立即关闭正在运行的应用，确定继续吗？'))
    return;
  const target = custom.value ? new Date(custom.value) : new Date(Date.now() + minutes * 60_000);
  if (target.getTime() <= Date.now()) return notify('请选择未来的时间', 'warning');
  pending.value = true;
  await nextTick();
  try {
    const plan = await invoke<Schedule>('create_schedule', {
      request: { action: action.value, targetTime: target.toISOString() },
    });
    activeUntil.value = new Date(plan.target_time);
    void refreshHistory();
    notify('计划已创建，Windows 计划任务正在托管', 'success');
  } catch (error) {
    notify(`创建失败：${String(error)}`, 'error');
  } finally {
    pending.value = false;
  }
}
async function adjust(delta: number) {
  if (!activeUntil.value || pending.value) return;
  pending.value = true;
  await nextTick();
  try {
    const plan = await invoke<Schedule>('adjust_schedule', { minutes: delta });
    activeUntil.value = new Date(plan.target_time);
    notify(delta > 0 ? '计划已增加 30 分钟' : '计划已减少 30 分钟', 'success');
  } catch (error) {
    notify(`更新失败：${String(error)}`, 'error');
  } finally {
    pending.value = false;
  }
}
async function cancel() {
  if (!activeUntil.value || pending.value) return;
  pending.value = true;
  await nextTick();
  try {
    await invoke('cancel_schedule');
    activeUntil.value = null;
    clearReminderTimers();
    void refreshHistory();
    notify('计划已取消', 'success');
  } catch (error) {
    notify(`取消失败：${String(error)}`, 'error');
  } finally {
    pending.value = false;
  }
}
async function saveSettings() {
  if (!settingsDirty.value) return notify('设置没有变化', 'info');
  try {
    await invoke('save_settings', {
      reminders: {
        ten_minutes: settings.value.reminders.ten,
        one_minute: settings.value.reminders.one,
        thirty_seconds: settings.value.reminders.thirty,
      },
      launchAtStartup: settings.value.startup,
      silentStartup: settings.value.silent,
      exitToTray: settings.value.exitToTray,
    });
    savedSettings.value = settingSnapshot();
    notify('设置已保存', 'success');
  } catch (error) {
    notify(`保存失败：${String(error)}`, 'error');
  }
}
function discardSettings() {
  if (!settingsDirty.value) return;
  settings.value = JSON.parse(savedSettings.value) as Settings;
}
async function minimizeWindow() {
  try {
    await invoke('minimize_window');
  } catch (error) {
    notify(`最小化失败：${String(error)}`, 'error');
  }
}
async function closeWindow() {
  try {
    await invoke(settings.value.exitToTray ? 'hide_window' : 'exit_application');
  } catch (error) {
    notify(`退出失败：${String(error)}`, 'error');
  }
}
async function hideToTray() {
  try {
    await invoke('hide_window');
  } catch (error) {
    notify(`隐藏失败：${String(error)}`, 'error');
  }
}
async function openRepository() {
  if (repositoryOpening.value) return;
  repositoryOpening.value = true;
  try {
    await openUrl(repositoryUrl);
  } catch (error) {
    const fallback = window.open(repositoryUrl, '_blank', 'noopener,noreferrer');
    if (!fallback) notify(`打开仓库失败：${String(error)}`, 'error');
  } finally {
    repositoryOpening.value = false;
  }
}
async function startWindowDrag(event: PointerEvent) {
  if (event.button !== 0) return;
  try {
    await getCurrentWindow().startDragging();
  } catch {}
}
async function openDataFile() {
  try {
    await openPath(diagnostics.value.data_file);
  } catch (error) {
    notify(`打开数据文件失败：${String(error)}`, 'error');
  }
}
async function checkForUpdate() {
  if (updateChecking.value) return;
  updateChecking.value = true;
  try {
    const response = await fetch(
      'https://api.github.com/repos/Zverly/NightOwlTimer/releases/latest',
      {
        headers: { Accept: 'application/vnd.github+json' },
      },
    );
    if (response.status === 404) {
      notify('仓库暂无已发布版本', 'info');
      return;
    }
    if (!response.ok) throw new Error(`GitHub API ${response.status}`);
    const release = (await response.json()) as { tag_name?: string; html_url?: string };
    const latest = (release.tag_name ?? '').replace(/^v/i, '');
    const current = String(appVersion).replace(/^v/i, '');
    if (latest && latest !== current) {
      notify(`发现新版本 v${latest}`, 'success');
      if (release.html_url) await openUrl(release.html_url);
    } else {
      notify('当前已是最新版本', 'success');
    }
  } catch (error) {
    notify(`检查更新失败：${String(error)}`, 'error');
  } finally {
    updateChecking.value = false;
  }
}
function formatDate(date: Date) {
  const pad = (value: number) => String(value).padStart(2, '0');
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}T${pad(date.getHours())}:${pad(date.getMinutes())}`;
}
function historyStatus(detail: string) {
  if (detail.includes('已执行')) return 'executed';
  if (detail.includes('执行失败')) return 'failed';
  if (detail.includes('已取消')) return 'cancelled';
  return 'scheduled';
}
function historyStatusLabel(detail: string) {
  const status = historyStatus(detail);
  return status === 'executed'
    ? '已执行'
    : status === 'failed'
      ? '执行失败'
      : status === 'cancelled'
        ? '已取消'
        : '已设定';
}

onMounted(async () => {
  try {
    const saved = await invoke<{
      reminders: {
        ten_minutes: boolean;
        one_minute: boolean;
        thirty_seconds: boolean;
      };
      launch_at_startup: boolean;
      silent_startup: boolean;
      exit_to_tray?: boolean;
    }>('get_settings');
    settings.value = {
      reminders: {
        ten: saved.reminders.ten_minutes,
        one: saved.reminders.one_minute,
        thirty: saved.reminders.thirty_seconds,
      },
      startup: saved.launch_at_startup,
      silent: saved.silent_startup,
      exitToTray: saved.exit_to_tray ?? true,
    };
  } catch {}
  savedSettings.value = settingSnapshot();
  try {
    const plan = await invoke<Schedule | null>('get_active_schedule');
    if (plan) {
      action.value = plan.action;
      activeUntil.value = new Date(plan.target_time);
    }
  } catch {}
  try {
    diagnostics.value = await invoke('get_diagnostics');
  } catch {}
  await refreshHistory();
  unlistenCancel = await listen('tray-cancel', cancel);
  unlistenAdjust = await listen<number>('tray-adjust', (event) => adjust(event.payload));
  unlistenSettings = await listen('tray-open-settings', () => {
    page.value = 'settings';
  });
  unlistenQuickSchedule = await listen<TrayQuickSchedule>('tray-quick-schedule', async (event) => {
    action.value = event.payload.action;
    custom.value = '';
    await schedule(event.payload.minutes);
  });
});
onBeforeUnmount(() => {
  window.clearInterval(timer);
  clearReminderTimers();
  unlistenCancel?.();
  unlistenAdjust?.();
  unlistenSettings?.();
  unlistenQuickSchedule?.();
});
watch(activeUntil, armReminders);
watch(
  () => settings.value.startup,
  (enabled) => {
    if (!enabled) settings.value.silent = false;
  },
);
</script>

<template>
  <main class="shell">
    <header class="topbar" data-tauri-drag-region>
      <div class="brand" data-tauri-drag-region @pointerdown="startWindowDrag">
        <span class="brand-mark"></span>
        <div><strong>NightOwl</strong><small>轻量定时助手</small></div>
      </div>
      <div class="title-drag" data-tauri-drag-region @pointerdown="startWindowDrag"></div>
      <div class="window-actions">
        <button type="button" title="最小化" aria-label="最小化" @click="minimizeWindow">
          <Minus :size="16" aria-hidden="true" /></button
        ><button type="button" class="close" title="退出" aria-label="退出" @click="closeWindow">
          <X :size="16" aria-hidden="true" />
        </button>
      </div>
    </header>
    <section class="content">
      <template v-if="page === 'timer'">
        <div class="page-entry-actions">
          <button type="button" class="page-entry" @click="page = 'history'">任务历史</button
          ><button type="button" class="page-entry" @click="page = 'settings'">
            <Settings :size="16" aria-hidden="true" />设置
          </button>
        </div>
        <section v-if="activeUntil" class="active-plan">
          <div>
            <p>当前计划 · {{ actionMeta.label }}</p>
            <strong>{{ countdown }}</strong
            ><small>将于 {{ targetText }} 执行，已由 Windows 计划任务托管</small>
          </div>
          <span class="running">● 运行中</span>
        </section>
        <section v-else class="hero">
          <div>
            <p>GOOD NIGHT, COMPUTER</p>
            <h1>让电脑在你需要的时刻<br /><em>安静下来。</em></h1>
            <small>选择动作和时间，NightOwl 会在后台可靠执行。</small>
          </div>
        </section>
        <section class="panel action-panel">
          <header><strong>执行动作</strong><small>选择一种方式</small></header>
          <div class="action-grid">
            <button
              v-for="item in actions"
              :key="item.id"
              class="action-card"
              :data-action="item.id"
              :class="[item.id, { selected: action === item.id }]"
              :aria-pressed="action === item.id"
              type="button"
              @click="action = item.id"
            >
              <span class="action-icon"
                ><component :is="item.icon" :size="20" aria-hidden="true" /></span
              ><span class="action-label">{{ item.label }}</span
              ><small>{{ item.description }}</small
              ><i v-if="action === item.id">已选择</i>
            </button>
          </div>
        </section>
        <section v-if="!activeUntil" class="panel time-panel">
          <header><strong>设置时间</strong><small>选择倒计时，或精确指定执行时刻</small></header>
          <div class="presets">
            <button
              v-for="item in [30, 60, 90, 120]"
              :key="item"
              :data-duration="item"
              :class="{ chosen: duration === item && !custom }"
              :aria-pressed="duration === item && !custom"
              type="button"
              @click="
                duration = item;
                custom = '';
              "
            >
              <b>{{ item }}</b
              ><small>分钟</small>
            </button>
          </div>
          <div class="exact-time">
            <div class="exact-copy">
              <Clock3 :size="16" aria-hidden="true" /><strong>精确时间</strong>
            </div>
            <input
              id="custom-time"
              :value="displayedDateTime"
              type="datetime-local"
              :min="formatDate(new Date())"
              @input="custom = ($event.target as HTMLInputElement).value"
            /><button
              data-testid="create-schedule"
              type="button"
              class="primary"
              :disabled="pending"
              @click="schedule()"
            >
              {{ pending ? '创建中…' : '创建计划' }}
            </button>
          </div>
          <div class="time-preview" aria-live="polite">
            <span>执行预览</span
            ><strong>{{
              custom
                ? `将于 ${customTargetText} 执行${actionMeta.label}`
                : `当前预设将在今天 ${presetTargetText} 执行${actionMeta.label}`
            }}</strong>
          </div>
        </section>
        <section v-else class="panel controls">
          <button type="button" :disabled="pending" @click="adjust(-30)">− 30 分钟</button
          ><button type="button" :disabled="pending" @click="adjust(30)">+ 30 分钟</button
          ><button type="button" class="cancel" :disabled="pending" @click="cancel">
            {{ pending ? '处理中…' : '取消计划' }}</button
          ><button type="button" class="ghost" :disabled="pending" @click="hideToTray">
            隐藏到托盘
          </button>
        </section>
      </template>
      <section v-else-if="page === 'history'" class="secondary-page">
        <button type="button" class="icon-back" @click="page = 'timer'">
          <ArrowLeft :size="16" aria-hidden="true" /><span>返回定时任务</span>
        </button>
        <section class="panel history-panel">
          <header>
            <div>
              <strong>任务历史</strong
              ><small
                >最近的已设定、已取消和已执行任务{{
                  history.length > 6 ? ' · 显示最近 6 条' : ''
                }}</small
              >
            </div>
            <button
              type="button"
              class="history-refresh"
              :class="{ spinning: historyRefreshing }"
              :disabled="historyRefreshing"
              aria-label="刷新任务历史"
              title="刷新任务历史"
              @click="refreshHistory"
            >
              <RefreshCw :size="16" aria-hidden="true" /><em>{{
                historyRefreshing ? '刷新中' : '刷新'
              }}</em>
            </button>
          </header>
          <p v-if="!history.length" class="empty">暂无记录</p>
          <div v-for="item in visibleHistory" :key="item.time + item.detail" class="history-row">
            <span :class="historyStatus(item.detail)"></span>
            <div>
              <strong>{{ item.action }}</strong
              ><small>{{ item.time }}</small>
            </div>
            <em :class="historyStatus(item.detail)">{{ historyStatusLabel(item.detail) }}</em>
          </div>
        </section>
      </section>
      <section v-else-if="page === 'settings'" class="secondary-page">
        <button type="button" class="icon-back" @click="page = 'timer'">
          <ArrowLeft :size="16" aria-hidden="true" /><span>返回定时任务</span>
        </button>
        <section class="panel settings-panel">
          <header><strong>设置</strong><small>保存后立即生效</small></header>
          <div class="setting">
            <div><strong>提前提醒</strong><small>在计划执行前发送本地提醒</small></div>
            <div class="reminders">
              <label><input v-model="settings.reminders.ten" type="checkbox" />10 分钟</label
              ><label><input v-model="settings.reminders.one" type="checkbox" />1 分钟</label
              ><label><input v-model="settings.reminders.thirty" type="checkbox" />30 秒</label>
            </div>
          </div>
          <div class="setting">
            <div><strong>开机自启动</strong><small>登录 Windows 后自动运行</small></div>
            <input v-model="settings.startup" type="checkbox" />
          </div>
          <div class="setting">
            <div><strong>静默启动</strong><small>启动时直接隐藏到系统托盘</small></div>
            <input v-model="settings.silent" :disabled="!settings.startup" type="checkbox" />
          </div>
          <div class="setting">
            <div>
              <strong>关闭时隐藏到托盘</strong
              ><small>点击右上角关闭按钮时保留后台计划，默认开启</small>
            </div>
            <input v-model="settings.exitToTray" type="checkbox" />
          </div>
          <button type="button" class="about-entry" @click="page = 'about'">
            <span><strong>关于 NightOwl</strong><small>版本、运行环境与数据目录</small></span
            ><b>›</b>
          </button>
          <div class="setting-actions">
            <button
              type="button"
              class="ghost-button"
              :disabled="!settingsDirty"
              @click="discardSettings"
            >
              放弃修改</button
            ><button type="button" class="primary" :disabled="!settingsDirty" @click="saveSettings">
              保存设置
            </button>
          </div>
        </section>
      </section>
      <section v-else class="secondary-page">
        <button type="button" class="icon-back" @click="page = 'settings'">
          <ArrowLeft :size="16" aria-hidden="true" /><span>返回设置</span>
        </button>
        <section class="about-layout">
          <section class="panel about-panel">
            <div class="about-hero">
              <div class="about-mark"><img src="/nightowl-icon.png" alt="NightOwl 应用图标" /></div>
              <h1>NightOwl Timer</h1>
              <p>陪你把电脑安静地交给夜晚。</p>
            </div>
            <div class="about-details">
              <div class="about-detail">
                <span>版本</span>
                <div class="version-value">
                  <strong>v{{ diagnostics.version }}</strong>
                  <button
                    type="button"
                    class="update-button"
                    :disabled="updateChecking"
                    @click="checkForUpdate"
                  >
                    {{ updateChecking ? '检查中…' : '检查更新' }}
                  </button>
                </div>
              </div>
              <div class="about-detail">
                <span>运行平台</span><strong>{{ diagnostics.platform }}</strong>
              </div>
              <div class="about-detail repository-setting">
                <span>代码仓库</span>
                <a
                  :href="repositoryUrl"
                  :aria-busy="repositoryOpening"
                  aria-label="打开 NightOwl GitHub 仓库"
                  @click.prevent="openRepository"
                >
                  {{ repositoryUrl }} <ExternalLink :size="14" aria-hidden="true" />
                </a>
              </div>
              <div class="about-detail">
                <span>开源协议</span><strong>MIT License · © 2026 Zverly</strong>
              </div>
              <div class="about-detail local-data-detail">
                <span>本地数据</span>
                <a href="#" aria-label="打开本地数据文件" @click.prevent="openDataFile">
                  {{ diagnostics.history_count }} 条历史记录 · {{ diagnostics.data_file }}
                  <ExternalLink :size="14" aria-hidden="true" />
                </a>
              </div>
            </div>
          </section>
        </section>
      </section>
    </section>
    <footer>
      <span><i></i>Windows 计划任务已连接</span><span>v{{ appVersion }} · 本地运行</span>
    </footer>
    <div v-if="toast" class="toast" role="status" aria-live="polite" :data-tone="toast.tone">
      {{ toast.message }}
    </div>
  </main>
</template>
