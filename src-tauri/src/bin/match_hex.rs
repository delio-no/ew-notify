//! Offline pattern matcher — no WinDivert / admin needed.
//!
//! Paste hex from sniff/`app.log` and check DEATH / STOP_FARM (or config patterns).
//!
//! Examples:
//!   cargo run -p ew-notify --bin match_hex -- 00FE06032C010000
//!   cargo run -p ew-notify --bin match_hex -- --packet <full-ip-packet-hex>
//!   cargo run -p ew-notify --bin match_hex -- --file dump.txt
//!   cargo run -p ew-notify --bin match_hex -- --wrap-tcp 00FE06032C010000

use anyhow::{anyhow, Result};
use ew_notify_lib::config::{self, AppConfig};
use ew_notify_lib::packet::{
    make_ipv4_tcp_packet, parse_hex, PacketAnalyzer,
};
use std::env;
use std::fs;
use std::path::PathBuf;

enum Mode {
    /// Hex is transport payload (search as-is + also wrap in fake TCP packet).
    Payload,
    /// Hex is a full IP packet (as WinDivert returns).
    Packet,
}

struct Args {
    mode: Mode,
    wrap_tcp: bool,
    hex: Option<String>,
    file: Option<PathBuf>,
    config: Option<PathBuf>,
    help: bool,
}

fn parse_args() -> Result<Args> {
    let mut mode = Mode::Payload;
    let mut wrap_tcp = false;
    let mut hex = None;
    let mut file = None;
    let mut config = None;
    let mut help = false;

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => help = true,
            "--packet" | "-p" => mode = Mode::Packet,
            "--wrap-tcp" => wrap_tcp = true,
            "--file" | "-f" => {
                let v = args
                    .next()
                    .ok_or_else(|| anyhow!("--file requires a path"))?;
                file = Some(PathBuf::from(v));
            }
            "--config" | "-c" => {
                let v = args
                    .next()
                    .ok_or_else(|| anyhow!("--config requires a path"))?;
                config = Some(PathBuf::from(v));
            }
            other if other.starts_with('-') => {
                return Err(anyhow!("unknown flag: {other}"));
            }
            other => {
                if hex.is_some() {
                    return Err(anyhow!("unexpected extra argument: {other}"));
                }
                hex = Some(other.to_string());
            }
        }
    }

    Ok(Args {
        mode,
        wrap_tcp,
        hex,
        file,
        config,
        help,
    })
}

fn print_help() {
    eprintln!(
        "\
match_hex — offline DEATH/STOP_FARM (and config) matcher

USAGE:
  match_hex [OPTIONS] <hex>
  match_hex [OPTIONS] --file <path>

OPTIONS:
  -p, --packet       Treat hex as full IP packet (WinDivert raw)
      --wrap-tcp     Also wrap payload in a fake IPv4+TCP packet and re-check
  -f, --file PATH    Read hex from file (spaces/newlines ok)
  -c, --config PATH  Patterns from this config.json (else cwd/default)
  -h, --help         Show help

EXAMPLES:
  match_hex 00FE06032C010000
  match_hex --wrap-tcp 4E00FE11006100750074006F006600610072006D000000
  match_hex --packet 4500....   # full IP dump from sniff
"
    );
}

fn load_patterns(config_path: Option<PathBuf>) -> Result<AppConfig> {
    if let Some(path) = config_path {
        let content = fs::read_to_string(&path)?;
        return Ok(serde_json::from_str(&content)?);
    }
    match config::load_config() {
        Ok(c) => Ok(c),
        Err(_) => Ok(AppConfig::default()),
    }
}

fn main() -> Result<()> {
    let args = parse_args()?;
    if args.help {
        print_help();
        return Ok(());
    }

    let raw = if let Some(path) = args.file {
        fs::read_to_string(path)?
    } else {
        args.hex
            .ok_or_else(|| anyhow!("pass hex bytes or --file (see --help)"))?
    };

    let bytes = parse_hex(&raw)?;
    let cfg = load_patterns(args.config)?;
    let analyzer = PacketAnalyzer::new(cfg.target.patterns.clone());

    eprintln!("Patterns ({}):", cfg.target.patterns.len());
    for p in &cfg.target.patterns {
        eprintln!("  - {}  [{}]", p.name, p.bytes);
    }
    eprintln!("Input: {} bytes\n", bytes.len());

    match args.mode {
        Mode::Packet => {
            let payload = PacketAnalyzer::extract_transport_payload(&bytes);
            eprintln!(
                "IP packet {} B → transport payload {} B",
                bytes.len(),
                payload.len()
            );
            if !payload.is_empty() {
                eprintln!("payload hex: {}", hex_preview(payload, 64));
            }
            report_match(analyzer.analyze(&bytes));
        }
        Mode::Payload => {
            // Direct substring search on raw bytes (same as analyze after extract)
            let direct = find_in_bytes(&analyzer, &bytes);
            eprintln!("Direct payload scan:");
            let matched = direct.is_some();
            report_match(direct);

            if args.wrap_tcp || !matched {
                let pkt = make_ipv4_tcp_packet(&bytes);
                eprintln!("\nWrapped as IPv4+TCP ({} B total):", pkt.len());
                report_match(analyzer.analyze(&pkt));
            }
        }
    }

    Ok(())
}

fn find_in_bytes(analyzer: &PacketAnalyzer, payload: &[u8]) -> Option<ew_notify_lib::config::Pattern> {
    // Reuse analyze via a synthetic TCP packet so logic stays single-sourced.
    analyzer.analyze(&make_ipv4_tcp_packet(payload))
}

fn report_match(m: Option<ew_notify_lib::config::Pattern>) {
    match m {
        Some(p) => {
            println!("MATCH: {} — {}", p.name, p.description);
            println!("pattern bytes: {}", p.bytes);
        }
        None => {
            println!("NO MATCH");
        }
    }
}

fn hex_preview(data: &[u8], max: usize) -> String {
    data.iter()
        .take(max)
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join("")
}
