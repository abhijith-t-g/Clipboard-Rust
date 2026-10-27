mod history;
mod ui;

use std::sync::{Arc, Mutex};
use gtk4::prelude::*;
use gtk4::Application;
use history::ClipboardHistory;

const APP_ID: &str = "com.clipboard.app";

fn main() {
    // 1. Initialize history with capacity bound of 50 items
    let history = Arc::new(Mutex::new(ClipboardHistory::new(50)));

    // Populate initial items (demonstrates deduplication and promotion)
    {
        let mut h = history.lock().unwrap();
        h.add("cargo run --release".to_string());
        h.add("https://github.com/abhijith-t-g/clipboard-rust".to_string());
        h.add("let app = Application::builder().build();".to_string());
        h.add("sudo apt install -y libgtk-4-dev".to_string());
        h.add("git commit -m 'Implement GTK4 UI and deduplication'".to_string());
        // Duplicate copy: should promote to the top without creating duplicate row
        h.add("https://github.com/abhijith-t-g/clipboard-rust".to_string());
    }

    // 2. Initialize GTK4 Application
    let app = Application::builder().application_id(APP_ID).build();

    let history_clone = history.clone();
    app.connect_activate(move |app| {
        ui::show_or_create_ui(app, history_clone.clone());
    });

    // 3. Start event loop
    app.run();
}
