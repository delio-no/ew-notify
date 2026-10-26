export interface Pattern {
  name: string;
  bytes: string;
  description: string;
}

export interface RegisteredUser {
  chat_id: string;
  username: string | null;
  first_name: string | null;
  enabled: boolean;
  allow_screenshot?: boolean;
}

export interface Config {
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
}

export interface PatternMatchResult {
  matched: boolean;
  name: string | null;
  description: string | null;
  pattern_bytes: string | null;
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
  | 'console';

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
    patterns: [],
  },
  notifications: {
    telegram_enabled: true,
    tray_enabled: true,
    cooldown_seconds: 60,
    burst_dedup_ms: 2000,
  },
  startup: {
    auto_start_monitoring: false,
    start_minimized_to_tray: false,
    close_to_tray: true,
  },
});
