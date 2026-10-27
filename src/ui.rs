use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use gtk4::gdk;
use gtk4::gio;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{
    Application, ApplicationWindow, Box as GtkBox, Button, CssProvider, EventControllerKey,
    Label, ListBox, ListBoxRow, Orientation, PolicyType, ScrolledWindow, SearchEntry,
};

use crate::history::ClipboardHistory;

// Windows 11 Fluent Dark styling
const CSS: &str = r#"
window {
    background-color: transparent;
}

.win-container {
    background-color: #202020;
    border-radius: 12px;
    border: 1px solid #383838;
    padding: 10px 12px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.5);
}

.win-header {
    margin-bottom: 6px;
}

.win-title {
    color: #ffffff;
    font-size: 13px;
    font-weight: 600;
}

.clear-btn {
    background-color: #2b2b2b;
    color: #a0a0a0;
    font-size: 11px;
    font-weight: 500;
    border-radius: 6px;
    padding: 2px 8px;
    border: 1px solid #383838;
    min-height: 22px;
}

.clear-btn:hover {
    background-color: #383838;
    color: #ffffff;
}

.search-input {
    background-color: #2b2b2b;
    color: #ffffff;
    border-radius: 6px;
    border: 1px solid #383838;
    padding: 4px 8px;
    font-size: 12px;
    margin-bottom: 8px;
    min-height: 28px;
}

.search-input:focus-within {
    border-color: #60cdff;
    box-shadow: 0 0 0 1px #60cdff;
}

.history-list {
    background-color: transparent;
}

.history-row {
    background-color: #2b2b2b;
    border-radius: 6px;
    margin-bottom: 5px;
    padding: 7px 10px;
    border: 1px solid transparent;
    transition: background-color 100ms ease;
}

.history-row:hover {
    background-color: #333333;
    border-color: #404040;
}

.history-row:selected {
    background-color: #383838;
    border-left: 3px solid #60cdff;
}

.item-content {
    color: #ffffff;
    font-size: 12px;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
    line-height: 1.3;
}

.item-meta {
    color: #888888;
    font-size: 10px;
    margin-top: 3px;
}

.footer-bar {
    border-top: 1px solid #303030;
    padding-top: 6px;
    margin-top: 4px;
}

.footer-hint {
    color: #777777;
    font-size: 10px;
}
"#;

// State holding our single overlay instance
thread_local! {
    static OVERLAY_WINDOW: RefCell<Option<ApplicationWindow>> = const { RefCell::new(None) };
    static SEARCH_ENTRY_REF: RefCell<Option<SearchEntry>> = const { RefCell::new(None) };
    static LIST_BOX_REF: RefCell<Option<ListBox>> = const { RefCell::new(None) };
    static COUNT_BADGE_REF: RefCell<Option<Button>> = const { RefCell::new(None) };
    static APP_HOLD_GUARD: RefCell<Option<gio::ApplicationHoldGuard>> = const { RefCell::new(None) };
    static TARGET_WINDOW_ID: RefCell<Option<u32>> = const { RefCell::new(None) };
}

pub fn show_or_create_ui(app: &Application, history: Arc<Mutex<ClipboardHistory>>) {
    // Save target window before our window takes focus
    let prev_active = crate::x11_helper::get_active_window();
    TARGET_WINDOW_ID.with(|t| *t.borrow_mut() = prev_active);

    // If window already exists, refresh it and bring to front
    if let Some(window) = OVERLAY_WINDOW.with(|w| w.borrow().clone()) {
        if let Some(list_box) = LIST_BOX_REF.with(|l| l.borrow().clone()) {
            populate_list(&list_box, &history.lock().unwrap());
        }
        if let Some(search) = SEARCH_ENTRY_REF.with(|s| s.borrow().clone()) {
            search.set_text("");
            search.grab_focus();
        }
        update_badge(&history.lock().unwrap());
        window.set_visible(true);
        window.present();
        glib::timeout_add_local_once(std::time::Duration::from_millis(30), || {
            crate::x11_helper::position_window_at_cursor(330, 420);
        });
        return;
    }

    // 1. Hold application guard so it runs as a background daemon even when window is hidden
    APP_HOLD_GUARD.with(|guard| {
        *guard.borrow_mut() = Some(app.hold());
    });

    // 2. Install Windows 11 Dark CSS
    if let Some(display) = gdk::Display::default() {
        let provider = CssProvider::new();
        provider.load_from_data(CSS);
        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );

        // 3. Setup Clipboard Listener Daemon (Phase 2):
        // Automatically capture whatever the user copies anywhere on system
        let clipboard = display.clipboard();
        let history_listener = history.clone();
        clipboard.connect_changed(move |clip| {
            let hist = history_listener.clone();
            clip.read_text_async(None::<&gio::Cancellable>, move |res| {
                if let Ok(Some(text)) = res {
                    let text = text.to_string();
                    if hist.lock().unwrap().add(text) {
                        // Refresh UI if visible
                        if let Some(list_box) = LIST_BOX_REF.with(|l| l.borrow().clone()) {
                            populate_list(&list_box, &hist.lock().unwrap());
                            update_badge(&hist.lock().unwrap());
                        }
                    }
                }
            });
        });
    }

    // Main layout
    let vbox = GtkBox::new(Orientation::Vertical, 0);
    vbox.add_css_class("win-container");

    // Header bar (Title + "Clear all" button)
    let header_box = GtkBox::new(Orientation::Horizontal, 6);
    header_box.add_css_class("win-header");

    let title_label = Label::builder()
        .label("Clipboard")
        .css_classes(["win-title"])
        .hexpand(true)
        .xalign(0.0)
        .build();
    header_box.append(&title_label);

    let clear_btn = Button::builder()
        .label(&format!("Clear all ({})", history.lock().unwrap().len()))
        .css_classes(["clear-btn"])
        .build();
    header_box.append(&clear_btn);
    vbox.append(&header_box);

    // Search input
    let search_entry = SearchEntry::builder()
        .placeholder_text("Search...")
        .css_classes(["search-input"])
        .build();
    vbox.append(&search_entry);

    // Scrollable list
    let list_box = ListBox::builder()
        .selection_mode(gtk4::SelectionMode::Single)
        .css_classes(["history-list"])
        .build();

    populate_list(&list_box, &history.lock().unwrap());

    let scrolled_window = ScrolledWindow::builder()
        .hscrollbar_policy(PolicyType::Never)
        .vscrollbar_policy(PolicyType::Automatic)
        .vexpand(true)
        .child(&list_box)
        .build();
    vbox.append(&scrolled_window);

    // Footer with clean hint text
    let footer_box = GtkBox::new(Orientation::Horizontal, 6);
    footer_box.add_css_class("footer-bar");
    let footer_hint = Label::builder()
        .label("Enter to paste  •  Del to remove  •  Esc to close")
        .css_classes(["footer-hint"])
        .xalign(0.0)
        .hexpand(true)
        .build();
    footer_box.append(&footer_hint);
    vbox.append(&footer_box);

    // Compact Windows 11 dimensions (330px x 420px)
    let window = ApplicationWindow::builder()
        .application(app)
        .title("Clipboard")
        .default_width(330)
        .default_height(420)
        .decorated(false)
        .child(&vbox)
        .build();

    // Auto-dismiss when user clicks outside (Focus-out / Blur)
    let window_focus = window.clone();
    window.connect_is_active_notify(move |win| {
        if !win.is_active() {
            window_focus.set_visible(false);
        }
    });

    // Clear all action
    let history_clear = history.clone();
    let list_box_clear = list_box.clone();
    let clear_btn_ref = clear_btn.clone();
    clear_btn.connect_clicked(move |_| {
        let mut hist = history_clear.lock().unwrap();
        hist.clear();
        clear_btn_ref.set_label("Clear all (0)");
        populate_list(&list_box_clear, &hist);
    });

    // Search filter
    let search_text = Rc::new(RefCell::new(String::new()));
    let search_text_clone = search_text.clone();

    list_box.set_filter_func(move |row| {
        let query = search_text_clone.borrow().to_lowercase();
        if query.is_empty() {
            return true;
        }
        if let Some(content) = get_row_content(row) {
            content.to_lowercase().contains(&query)
        } else {
            true
        }
    });

    let list_box_filter = list_box.clone();
    search_entry.connect_search_changed(move |entry| {
        *search_text.borrow_mut() = entry.text().to_string();
        list_box_filter.invalidate_filter();
    });

    // Activate item: Copy & Hide
    let window_activate = window.clone();
    let history_activate = history.clone();
    let activate_selected = move |row: &ListBoxRow| {
        if let Some(content) = get_row_content(row) {
            if let Some(display) = gdk::Display::default() {
                display.clipboard().set_text(&content);
            }
            history_activate.lock().unwrap().add(content);
            window_activate.set_visible(false);
            let target_win = TARGET_WINDOW_ID.with(|t| *t.borrow());
            crate::x11_helper::paste_into_window(target_win);
        }
    };

    let activate_clone = activate_selected.clone();
    list_box.connect_row_activated(move |_, row| {
        activate_clone(row);
    });

    // Keyboard shortcuts
    let key_controller = EventControllerKey::new();
    let window_keys = window.clone();
    let history_keys = history.clone();
    let list_box_keys = list_box.clone();

    key_controller.connect_key_pressed(move |_, key, _, _| {
        match key {
            gdk::Key::Escape => {
                window_keys.set_visible(false);
                glib::Propagation::Stop
            }
            gdk::Key::Delete => {
                if let Some(row) = list_box_keys.selected_row() {
                    let idx = row.index();
                    if idx >= 0 {
                        let mut hist = history_keys.lock().unwrap();
                        hist.remove_at(idx as usize);
                        update_badge(&hist);
                        populate_list(&list_box_keys, &hist);
                    }
                }
                glib::Propagation::Stop
            }
            gdk::Key::Return | gdk::Key::KP_Enter => {
                if let Some(row) = list_box_keys.selected_row() {
                    activate_selected(&row);
                    glib::Propagation::Stop
                } else {
                    glib::Propagation::Proceed
                }
            }
            _ => glib::Propagation::Proceed,
        }
    });

    window.add_controller(key_controller);

    // Save references to state
    OVERLAY_WINDOW.with(|w| *w.borrow_mut() = Some(window.clone()));
    SEARCH_ENTRY_REF.with(|s| *s.borrow_mut() = Some(search_entry.clone()));
    LIST_BOX_REF.with(|l| *l.borrow_mut() = Some(list_box));
    COUNT_BADGE_REF.with(|b| *b.borrow_mut() = Some(clear_btn));

    search_entry.grab_focus();
    window.set_visible(true);
    window.present();
    glib::timeout_add_local_once(std::time::Duration::from_millis(50), || {
        crate::x11_helper::position_window_at_cursor(330, 420);
    });
}

fn update_badge(history: &ClipboardHistory) {
    if let Some(btn) = COUNT_BADGE_REF.with(|b| b.borrow().clone()) {
        btn.set_label(&format!("Clear all ({})", history.len()));
    }
}

fn populate_list(list_box: &ListBox, history: &ClipboardHistory) {
    while let Some(child) = list_box.first_child() {
        list_box.remove(&child);
    }

    for item in history.get_items().iter() {
        let row = ListBoxRow::builder()
            .css_classes(["history-row"])
            .build();

        let row_box = GtkBox::new(Orientation::Vertical, 2);

        // Preview snippet (max 3 lines)
        let lines: Vec<&str> = item.content.lines().take(3).collect();
        let preview = lines.join("\n");

        let content_label = Label::builder()
            .label(&preview)
            .css_classes(["item-content"])
            .xalign(0.0)
            .hexpand(true)
            .ellipsize(gtk4::pango::EllipsizeMode::End)
            .max_width_chars(35)
            .wrap(true)
            .lines(3)
            .build();
        row_box.append(&content_label);

        // Metadata: relative time and character length
        let meta_text = format!("{} • {} chars", item.relative_time(), item.content.len());
        let meta_label = Label::builder()
            .label(&meta_text)
            .css_classes(["item-meta"])
            .xalign(0.0)
            .build();
        row_box.append(&meta_label);

        unsafe {
            row.set_data("clipboard-content", item.content.clone());
        }

        row.set_child(Some(&row_box));
        list_box.append(&row);
    }

    if let Some(first_row) = list_box.row_at_index(0) {
        list_box.select_row(Some(&first_row));
    }
}

fn get_row_content(row: &ListBoxRow) -> Option<String> {
    unsafe { row.data::<String>("clipboard-content").map(|s| s.as_ref().clone()) }
}
