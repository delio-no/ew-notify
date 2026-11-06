export interface Pattern {
  name: string;
  bytes: string;
  description: string;
  notify_enabled?: boolean;
  screenshot_enabled?: boolean;
}

export interface RegisteredUser {
  chat_id: string;
  username: string | null;
  first_name: string | null;
  enabled: boolean;
  allow_screenshot?: boolean;
}

export type ClickerTarget =
  | { type: 'Captured' }
  | { type: 'Slot'; data: number }
  | { type: 'All' };

export interface ClickerRule {
  id: string;
  name: string;
  enabled: boolean;
  key: string;
  interval_ms: number;
  target: ClickerTarget;
  stop_on_pattern: boolean;
  stop_patterns: string[];
  start_on_pattern: boolean;
  start_patterns: string[];
}

export interface SlotNotificationConfig {
  notify_on_stop: boolean;
  notify_on_start: boolean;
}

export interface ClickerConfig {
  rules: ClickerRule[];
  notify_on_stop: boolean;
  notify_on_start?: boolean;
  slot_notifications?: Record<string, SlotNotificationConfig>;
}

export interface ClickerStatus {
  running: boolean;
  count: number;
  last_stop_reason?: string | null;
  config: ClickerConfig;
}

export interface UpdaterConfig {
  check_on_startup: boolean;
  ignored_version?: string | null;
}

export interface UpdateInfo {
  current_version: string;
  latest_version: string;
  has_update: boolean;
  release_notes: string;
  published_at: string;
  download_url: string;
  asset_name: string;
}

export interface Config {
  schema_version?: number;
  telegram: {
    bot_token: string;
    admin_chat_id?: string;
    auto_start: boolean;
    registered_users: RegisteredUser[];
    notify_all_users: boolean;
  };
  target: {
    process_name: string;
    capture_mode?: 'auto' | 'manual';
    tcp_port: number | null;
    udp_port: number | null;
    remote_ip?: string | null;
    patterns: Pattern[];
  };
  notifications: {
    telegram_enabled: boolean;
    tray_enabled: boolean;
    cooldown_seconds: number;
    burst_dedup_ms?: number;
    screenshot_delay_ms?: number;
  };
  startup: {
    auto_start_monitoring: boolean;
    start_minimized_to_tray: boolean;
    close_to_tray: boolean;
  };
  clicker?: ClickerConfig;
  updater?: UpdaterConfig;
}

export interface MonitoringStatus {
  active: boolean;
  process_name: string;
  tcp_port: number | null;
  filter: string;
  packets_seen: number;
  matches_seen: number;
  dedup_suppressed: number;
  process_filter_active: boolean;
  bot_running: boolean;
  last_error: string;
  is_admin: boolean;
  config_path: string;
  log_path: string;
  capture_mode: string;
  discovered_endpoints: string[];
  packet_log_enabled: boolean;
  packet_log_skipped: number;
}

export interface PacketLogEntry {
  time: string;
  remote: string;
  local: string;
  len: number;
  hex: string;
  pid?: number | null;
  slot?: number | null;
  window_title?: string | null;
}

export interface PatternMatchResult {
  matched: boolean;
  name: string | null;
  description: string | null;
  pattern_bytes: string | null;
}

export interface TrackedWindowInfo {
  slot: number;
  pid: number;
  hwnd: number;
  title: string;
}

export type WindowFilterMode =
  | { mode: 'All' }
  | { mode: 'Single'; data: TrackedWindowInfo }
  | { mode: 'Multi'; data: TrackedWindowInfo[] };

export interface TargetWindow {
  hwnd: number;
  pid: number;
  title: string;
  is_minimized: boolean;
  width: number;
  height: number;
}

export interface StoredNotification {
  title: string;
  body: string;
  time: string;
}

export type PageId =
  | 'dashboard'
  | 'settings'
  | 'patterns'
  | 'notifications'
  | 'users'
  | 'console'
  | 'clicker';

export const defaultConfig = (): Config => ({
  telegram: {
    bot_token: '',
    admin_chat_id: '',
    auto_start: false,
    registered_users: [],
    notify_all_users: true,
  },
  target: {
    process_name: 'EndlessWar.exe',
    capture_mode: 'auto',
    tcp_port: 7777,
    udp_port: null,
    remote_ip: null,
    patterns: [
      {
        name: 'DEATH',
        bytes: '00FE06032C010000',
        description: 'Персонаж умер',
        notify_enabled: true,
        screenshot_enabled: true,
      },
      {
        name: 'STOP_FARM',
        bytes: '4E00FE11006100750074006F006600610072006D000000',
        description: 'Автофарм остановлен',
        notify_enabled: true,
        screenshot_enabled: true,
      },
    ],
  },
  notifications: {
    telegram_enabled: true,
    tray_enabled: true,
    cooldown_seconds: 10,
    burst_dedup_ms: 2000,
    screenshot_delay_ms: 1500,
  },
  startup: {
    auto_start_monitoring: false,
    start_minimized_to_tray: false,
    close_to_tray: true,
  },
  clicker: {
    rules: [
      {
        id: 'rule-1',
        name: 'Клавиша 1',
        enabled: true,
        key: '1',
        interval_ms: 1000,
        target: { type: 'Slot', data: 1 },
        stop_on_pattern: false,
        stop_patterns: [],
        start_on_pattern: false,
        start_patterns: [],
      },
    ],
    notify_on_stop: false,
    notify_on_start: false,
    slot_notifications: {},
  },
  updater: {
    check_on_startup: true,
    ignored_version: null,
  },
});
