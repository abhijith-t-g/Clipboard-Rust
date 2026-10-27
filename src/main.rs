mod history;
mod ui;
mod x11_helper;

use std::sync::{Arc, Mutex};
use gtk4::prelude::*;
use gtk4::Application;
use history::ClipboardHistory;

const APP_ID: &str = "com.clipboard.app";

fn main() {
    // 1. Initialize history with capacity bound of 50 items
    let history = Arc::new(Mutex::new(ClipboardHistory::new(50)));



    // 2. Initialize GTK4 Application
    let app = Application::builder().application_id(APP_ID).build();

    let history_clone = history.clone();
    app.connect_activate(move |app| {
        ui::show_or_create_ui(app, history_clone.clone());
    });

    // 3. Start event loop
    app.run();
}
