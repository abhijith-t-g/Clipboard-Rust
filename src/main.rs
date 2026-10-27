mod history;
mod ui;
mod x11_helper;

use std::env;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use gtk4::prelude::*;
use gtk4::Application;
use history::ClipboardHistory;

const APP_ID: &str = "com.clipboard.app";
static SKIP_FIRST_ACTIVATE: AtomicBool = AtomicBool::new(false);

fn main() {
    let raw_args: Vec<String> = env::args().collect();
    let is_daemon = raw_args.iter().any(|arg| arg == "--daemon");
    if is_daemon {
        SKIP_FIRST_ACTIVATE.store(true, Ordering::SeqCst);
    }

    // Filter out --daemon before passing to GTK so it doesn't parse it as an unknown option
    let gtk_args: Vec<String> = raw_args
        .into_iter()
        .filter(|arg| arg != "--daemon")
        .collect();

    // 1. Initialize history with capacity bound of 50 items
    let history = Arc::new(Mutex::new(ClipboardHistory::new(50)));

    // 2. Initialize GTK4 Application
    let app = Application::builder().application_id(APP_ID).build();

    let history_startup = history.clone();
    app.connect_startup(move |app| {
        ui::init_daemon(app, history_startup.clone());
    });

    let history_activate = history.clone();
    app.connect_activate(move |app| {
        if SKIP_FIRST_ACTIVATE.swap(false, Ordering::SeqCst) {
            // Started with --daemon: keep running quietly in the background!
            return;
        }
        ui::show_or_create_ui(app, history_activate.clone());
    });

    // 3. Start event loop with filtered arguments
    app.run_with_args(&gtk_args);
}
