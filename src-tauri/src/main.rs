#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    std::panic::set_hook(Box::new(|info| {
        eprintln!("PANIC: {:?}", info);
        let _ = std::fs::write("panic.log", format!("{:?}", info));
    }));
    ew_notify_lib::run();
}
