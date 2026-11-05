import { invoke } from '@tauri-apps/api/core';
import type {
  ClickerConfig,
  ClickerStatus,
  Config,
  MonitoringStatus,
  PacketLogEntry,
  Pattern,
  PatternAction,
  PatternMatchResult,
  RegisteredUser,
  StoredNotification,
  TargetWindow,
  WindowFilterMode,
} from './types';

export const api = {
  getConfig: () => invoke<Config>('get_config'),
  updateConfig: (newConfig: Config) =>
    invoke<string>('update_config', { newConfig }),
  resetConfig: () => invoke<Config>('reset_config_to_defaults'),

  getMonitoringStatus: () => invoke<MonitoringStatus>('get_monitoring_status'),
  startMonitoring: () => invoke<string>('start_monitoring'),
  stopMonitoring: () => invoke('stop_monitoring'),
  hideToTray: () => invoke('hide_to_tray'),

  getRecentNotifications: () =>
    invoke<StoredNotification[]>('get_recent_notifications'),
  getRegisteredUsers: () => invoke<RegisteredUser[]>('get_registered_users'),
  setUserScreenshotAllowed: (chatId: string, allowed: boolean) =>
    invoke<string>('set_user_screenshot_allowed', { chatId, allowed }),
  setUserEnabled: (chatId: string, enabled: boolean) =>
    invoke<string>('set_user_enabled', { chatId, enabled }),
  setNotifyAllUsers: (enabled: boolean) =>
    invoke<string>('set_notify_all_users', { enabled }),

  getRecentLogs: () =>
    invoke<Array<{ time: string; message: string }>>('get_recent_logs'),
  clearLogs: () => invoke('clear_logs'),
  getRecentPackets: () => invoke<PacketLogEntry[]>('get_recent_packets'),
  clearPacketLog: () => invoke('clear_packet_log'),
  setPacketLogEnabled: (enabled: boolean) =>
    invoke('set_packet_log_enabled', { enabled }),

  addPattern: (
    name: string,
    bytes: string,
    description: string,
    action?: PatternAction,
    notify_enabled?: boolean
  ) =>
    invoke('add_pattern', {
      name,
      bytes,
      description,
      action,
      notifyEnabled: notify_enabled ?? true,
    }),
  setPatternNotifyEnabled: (name: string, enabled: boolean) =>
    invoke<string>('set_pattern_notify_enabled', { name, enabled }),
  setPatternActionEnabled: (name: string, enabled: boolean) =>
    invoke<string>('set_pattern_action_enabled', { name, enabled }),
  setPatternAction: (name: string, action?: PatternAction) =>
    invoke<string>('set_pattern_action', { name, action }),
  removePattern: (name: string) => invoke('remove_pattern', { name }),
  resetPatterns: () => invoke<Pattern[]>('reset_patterns_to_defaults'),
  exportPatterns: () => invoke<string>('export_patterns'),
  importPatterns: (json: string, replace: boolean) =>
    invoke<Pattern[]>('import_patterns', { json, replace }),
  testPatternHex: (hex: string) =>
    invoke<PatternMatchResult>('test_pattern_hex', { hex }),
  testSendKey: (key: string) => invoke<string>('test_send_key', { key }),

  testTelegram: (token: string) => invoke<string>('test_telegram', { token }),
  startTelegramBot: (token: string) =>
    invoke<string>('start_telegram_bot', { token }),
  stopTelegramBot: () => invoke('stop_telegram_bot'),
  openUrl: (url: string) => invoke('open_url', { url }),

  // Selective window filtering
  getWindowFilterStatus: () =>
    invoke<WindowFilterMode>('get_window_filter_status'),
  setWindowFilter: (slot: number, pid: number) =>
    invoke<WindowFilterMode>('set_window_filter', { slot, pid }),
  resetWindowFilter: () =>
    invoke<WindowFilterMode>('reset_window_filter'),
  getTargetWindows: () =>
    invoke<TargetWindow[]>('get_target_windows'),

  // Auto-clicker
  startClicker: () => invoke<string>('start_clicker'),
  stopClicker: () => invoke<string>('stop_clicker'),
  getClickerStatus: () => invoke<ClickerStatus>('get_clicker_status'),
  saveClickerConfig: (clickerConfig: ClickerConfig) =>
    invoke<string>('save_clicker_config', { clickerConfig }),
};
