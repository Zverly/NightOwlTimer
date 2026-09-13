import { flushPromises, mount } from '@vue/test-utils';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { nextTick } from 'vue';

const { invokeMock, listenMock, openUrlMock, openPathMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  listenMock: vi.fn(async () => () => undefined),
  openUrlMock: vi.fn(async () => undefined),
  openPathMock: vi.fn(async () => undefined),
}));

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));
vi.mock('@tauri-apps/api/event', () => ({ listen: listenMock }));
vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({ startDragging: vi.fn(async () => undefined) }),
}));
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: openUrlMock, openPath: openPathMock }));
vi.mock('/nightowl-icon.png', () => ({ default: '' }));

function defaultInvoke(command: string) {
  if (command === 'get_settings')
    return Promise.resolve({
      reminders: { ten_minutes: true, one_minute: true, thirty_seconds: true },
      launch_at_startup: false,
      silent_startup: false,
      exit_to_tray: true,
    });
  if (command === 'get_active_schedule') return Promise.resolve(null);
  if (command === 'get_diagnostics')
    return Promise.resolve({
      version: '0.1.1',
      platform: 'Windows',
      history_count: 0,
      data_directory: 'E:/data',
      data_file: 'E:/data/nightowl-data.json',
    });
  if (command === 'get_history') return Promise.resolve([]);
  return Promise.resolve(undefined);
}

async function mountApp() {
  const { default: App } = await import('./App.vue');
  const wrapper = mount(App);
  await flushPromises();
  return wrapper;
}

describe('App', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockImplementation(defaultInvoke);
    listenMock.mockClear();
    openUrlMock.mockClear();
    openPathMock.mockClear();
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: { writeText: vi.fn(async () => undefined) },
    });
  });

  it('exposes the topbar drag region and accessible window buttons', async () => {
    const wrapper = await mountApp();
    expect(wrapper.get('header.topbar').attributes('data-tauri-drag-region')).toBe('');
    expect(wrapper.get('.brand').attributes('data-tauri-drag-region')).toBe('');
    expect(wrapper.get('.title-drag').attributes('data-tauri-drag-region')).toBe('');
    expect(wrapper.get('button[aria-label="最小化"]').attributes('title')).toBe('最小化');
    expect(wrapper.get('button[aria-label="退出"]').attributes('title')).toBe('退出');
    expect(wrapper.find('.window-actions').attributes('data-tauri-drag-region')).toBeUndefined();
    await wrapper.get('.title-drag').trigger('pointerdown');
  });

  it('renders Lucide SVG icons and toggles action and 60-minute preset state', async () => {
    const wrapper = await mountApp();
    expect(wrapper.findAll('svg').length).toBeGreaterThan(0);
    const force = wrapper.get('[data-action="force"]');
    expect(force.attributes('aria-pressed')).toBe('false');
    await force.trigger('click');
    expect(force.attributes('aria-pressed')).toBe('true');

    const preset60 = wrapper.get('[data-duration="60"]');
    expect(preset60.attributes('aria-pressed')).toBe('true');
    await wrapper.get('[data-duration="90"]').trigger('click');
    expect(preset60.attributes('aria-pressed')).toBe('false');
    expect(wrapper.get('[data-duration="90"]').attributes('aria-pressed')).toBe('true');
  });

  it('shows the fixed GitHub URL and opens or copies it', async () => {
    const wrapper = await mountApp();
    await wrapper.findAll('button.page-entry')[1].trigger('click');
    await wrapper.get('button.about-entry').trigger('click');
    const repository = wrapper.get('a[href="https://github.com/Zverly/NightOwlTimer"]');
    expect(repository.text()).toBe('https://github.com/Zverly/NightOwlTimer');
    await repository.trigger('click');
    await flushPromises();
    expect(invokeMock).toHaveBeenCalledWith('open_repository');
    await wrapper.get('.local-data-detail a').trigger('click');
    await flushPromises();
    expect(invokeMock).toHaveBeenCalledWith('open_data_file');
  });

  it('checks GitHub for the latest release', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => ({ ok: true, status: 200, json: async () => ({ tag_name: 'v1.0.0' }) })),
    );
    const wrapper = await mountApp();
    await wrapper.findAll('button.page-entry')[1].trigger('click');
    await wrapper.get('button.about-entry').trigger('click');
    await wrapper.get('.update-button').trigger('click');
    await flushPromises();
    expect(fetch).toHaveBeenCalledWith(
      'https://api.github.com/repos/Zverly/NightOwlTimer/releases/latest',
      expect.objectContaining({ headers: expect.any(Object) }),
    );
    expect(wrapper.get('.toast').text()).toContain('当前已是最新版本');
    vi.unstubAllGlobals();
  });

  it('disables the create button while scheduling is pending', async () => {
    let resolveSchedule!: (value: { target_time: string }) => void;
    const schedulePromise = new Promise<{ target_time: string }>((resolve) => {
      resolveSchedule = resolve;
    });
    invokeMock.mockImplementation((command: string) =>
      command === 'create_schedule' ? schedulePromise : defaultInvoke(command),
    );
    const wrapper = await mountApp();
    const createButton = wrapper.get('[data-testid="create-schedule"]');
    await createButton.trigger('click');
    await nextTick();
    expect(createButton.attributes('disabled')).toBeDefined();
    expect(createButton.text()).toContain('创建中');
    resolveSchedule({ target_time: new Date(Date.now() + 60 * 60_000).toISOString() });
    await flushPromises();
    expect(wrapper.find('[data-testid="create-schedule"]').exists()).toBe(false);
  });
});
