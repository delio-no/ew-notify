//! Debug packet sniffer (Windows / WinDivert sniff mode).
//!
//! Usage:
//!   cargo run -p ew-notify --bin sniff -- [--port 7777] [--ip 192.168.x.x] [--matches-only]
//!
//! Port defaults to `target.tcp_port` from config.json (cwd or next to exe).

use anyhow::{anyhow, Result};
use ew_notify_lib::config::{self, AppConfig};
use ew_notify_lib::packet::{build_filter, PacketAnalyzer, PacketCapture};
use std::env;
use std::time::Instant;

struct Args {
    port: Option<u16>,
    ip: Option<String>,
    matches_only: bool,
    help: bool,
}

fn parse_args() -> Result<Args> {
    let mut port = None;
    let mut ip = None;
    let mut matches_only = false;
    let mut help = false;

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => help = true,
            "--matches-only" | "-m" => matches_only = true,
            "--port" | "-p" => {
                let v = args
                    .next()
                    .ok_or_else(|| anyhow!("--port requires a value"))?;
                port = Some(v.parse::<u16>()?);
            }
            "--ip" | "-i" => {
                let v = args
                    .next()
                    .ok_or_else(|| anyhow!("--ip requires a value"))?;
                ip = Some(v);
            }
            other if other.starts_with('-') => {
                return Err(anyhow!("unknown flag: {}", other));
            }
            other => {
                return Err(anyhow!("unexpected argument: {}", other));
            }
        }
    }

    Ok(Args {
        port,
        ip,
        matches_only,
        help,
    })
}

fn print_help() {
    eprintln!(
        "ew-notify sniff — WinDivert packet sniffer (sniff mode, no reinject)

USAGE:
  sniff [OPTIONS]

OPTIONS:
  -p, --port <PORT>     Game server TCP port (remote). Default: config.json target.tcp_port
  -i, --ip <ADDR>       Optional server IP (ip.SrcAddr), e.g. 157.180.3.56
  -m, --matches-only    Print only packets matching DEATH / STOP_FARM (or config patterns)
  -h, --help            Show this help

Filter: inbound and tcp.SrcPort == <port> [and ip.SrcAddr == <ip>]
Config is loaded from ./config.json if present, otherwise next to the executable.
"
    );
}

fn load_config_flexible() -> AppConfig {
    // Prefer workspace/cwd config for CLI use
    if let Ok(cwd) = env::current_dir() {
        let path = cwd.join("config.json");
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(cfg) = serde_json::from_str::<AppConfig>(&content) {
                    eprintln!("Loaded config: {}", path.display());
                    return cfg;
                }
            }
        }
    }

    match config::load_config() {
        Ok(cfg) => {
            eprintln!("Loaded config via app path");
            cfg
        }
        Err(_) => {
            eprintln!("No config found, using defaults");
            AppConfig::default()
        }
    }
}

fn hex_dump(data: &[u8], max: usize) -> String {
    data.iter()
        .take(max)
        .map(|b| format!("{:02X}", b))
        .collect::<Vec<_>>()
        .join(" ")
}

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let args = parse_args()?;
    if args.help {
        print_help();
        return Ok(());
    }

    #[cfg(not(windows))]
    {
        let _ = args;
        return Err(anyhow!("sniff is only supported on Windows"));
    }

    #[cfg(windows)]
    {
        let config = load_config_flexible();
        let port = args
            .port
            .or(config.target.tcp_port)
            .unwrap_or(7777);
        let ip = args.ip.as_deref();
        let filter = build_filter(Some(port), None, ip);

        eprintln!("Filter: {filter}");
        eprintln!("Patterns: {}", config.target.patterns.len());
        for p in &config.target.patterns {
            eprintln!("  - {} ({})", p.name, p.description);
        }
        eprintln!("Ctrl+C to stop\n");

        let analyzer = PacketAnalyzer::new(config.target.patterns.clone());
        let capture = PacketCapture::new(Some(port), None, ip)?;
        let started = Instant::now();
        let mut count: u64 = 0;
        let mut match_count: u64 = 0;

        loop {
            let raw = capture.read_packet()?;
            count += 1;

            let payload = PacketAnalyzer::extract_transport_payload(&raw);
            let matched = analyzer.analyze(&raw);

            if args.matches_only && matched.is_none() {
                continue;
            }

            let elapsed = started.elapsed().as_secs_f32();
            if let Some(ref pat) = matched {
                match_count += 1;
                println!(
                    "[{elapsed:8.2}s #{count}] MATCH {} — {} | payload {} B",
                    pat.name,
                    pat.description,
                    payload.len()
                );
            } else {
                println!(
                    "[{elapsed:8.2}s #{count}] packet {} B | payload {} B",
                    raw.len(),
                    payload.len()
                );
            }

            if !payload.is_empty() {
                let dump_len = if matched.is_some() {
                    payload.len().min(256)
                } else {
                    payload.len().min(64)
                };
                println!("  payload: {}", hex_dump(payload, dump_len));
                if payload.len() > dump_len {
                    println!("  ... ({} more bytes)", payload.len() - dump_len);
                }
            } else {
                println!("  (empty payload) raw: {}", hex_dump(&raw, 32));
            }

            if matched.is_some() {
                println!("  matches so far: {match_count}");
            }
            println!();
        }
    }
}
