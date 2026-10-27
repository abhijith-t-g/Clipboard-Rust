use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use gtk4::gdk;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{
    Application, ApplicationWindow, Box as GtkBox, CssProvider, EventControllerKey,
    Label, ListBox, ListBoxRow, Orientation, PolicyType, ScrolledWindow, SearchEntry,
};

use crate::history::ClipboardHistory;

const CSS: &str = r#"
window {
    background-color: transparent;
}

.main-container {
    background-color: #1e1e2e;
    border-radius: 16px;
    border: 1px solid #313244;
    padding: 14px;
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.4);
}

.header-title {
    color: #cdd6f4;
    font-size: 15px;
    font-weight: 700;
    margin-bottom: 4px;
}

.header-badge {
    background-color: #313244;
    color: #89b4fa;
    font-size: 11px;
    font-weight: 600;
    border-radius: 6px;
    padding: 2px 8px;
}

.search-input {
    background-color: #181825;
    color: #cdd6f4;
    border-radius: 10px;
    border: 1px solid #313244;
    padding: 8px 12px;
    font-size: 13px;
    margin-top: 6px;
    margin-bottom: 8px;
}

.search-input:focus-within {
    border-color: #89b4fa;
    box-shadow: 0 0 0 2px rgba(137, 180, 250, 0.25);
}

.history-list {
    background-color: transparent;
    border-radius: 10px;
}

.history-row {
    background-color: #181825;
    border-radius: 8px;
    margin-bottom: 6px;
    padding: 8px 12px;
    border: 1px solid transparent;
    transition: all 150ms ease-in-out;
}

.history-row:hover {
    background-color: #24273a;
    border-color: #363a4f;
}

.history-row:selected {
    background-color: #313244;
    border-left: 3px solid #89b4fa;
}

.item-index {
    color: #89b4fa;
    font-size: 11px;
    font-weight: 700;
    margin-right: 6px;
}

.item-content {
    color: #cdd6f4;
    font-size: 13px;
    font-family: monospace;
}

.item-meta {
    color: #6c7086;
    font-size: 11px;
    margin-top: 3px;
}

.footer-bar {
    border-top: 1px solid #313244;
    padding-top: 8px;
    margin-top: 6px;
}

.footer-hint {
    color: #6c7086;
    font-size: 11px;
}

.footer-key {
    background-color: #313244;
    color: #a6adc8;
    border-radius: 4px;
    padding: 1px 5px;
    font-size: 10px;
    font-weight: 600;
}
"#;

pub fn build_ui(app: &Application, history: Arc<Mutex<ClipboardHistory>>) {
    // 1. Install modern dark theme CSS
    if let Some(display) = gdk::Display::default() {
        let provider = CssProvider::new();
        provider.load_from_data(CSS);
        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }

    // Main layout container with styling
    let vbox = GtkBox::new(Orientation::Vertical, 0);
    vbox.add_css_class("main-container");

    // Header bar (Title + Item Count Badge)
    let header_box = GtkBox::new(Orientation::Horizontal, 8);
    let title_label = Label::builder()
        .label("Clipboard History")
        .css_classes(["header-title"])
        .hexpand(true)
        .xalign(0.0)
        .build();
    header_box.append(&title_label);

    let count_badge = Label::builder()
        .label(&format!("{} items", history.lock().unwrap().len()))
        .css_classes(["header-badge"])
        .build();
    header_box.append(&count_badge);
    vbox.append(&header_box);

    // Search bar
    let search_entry = SearchEntry::builder()
        .placeholder_text("Type to search history...")
        .css_classes(["search-input"])
        .build();
    vbox.append(&search_entry);

    // ListBox for items
    let list_box = ListBox::builder()
        .selection_mode(gtk4::SelectionMode::Single)
        .css_classes(["history-list"])
        .build();

    // Populate rows from history
    populate_list(&list_box, &history.lock().unwrap());

    // Wrap list inside a scrollable view
    let scrolled_window = ScrolledWindow::builder()
        .hscrollbar_policy(PolicyType::Never)
        .vscrollbar_policy(PolicyType::Automatic)
        .vexpand(true)
        .child(&list_box)
        .build();
    vbox.append(&scrolled_window);

    // Footer with key hints
    let footer_box = GtkBox::new(Orientation::Horizontal, 8);
    footer_box.add_css_class("footer-bar");
    let footer_hint = Label::builder()
        .label(
            "<span class='footer-key'>Enter</span> Copy &amp; Close   \
             <span class='footer-key'>Del</span> Delete   \
             <span class='footer-key'>Esc</span> Dismiss",
        )
        .use_markup(true)
        .css_classes(["footer-hint"])
        .xalign(0.0)
        .hexpand(true)
        .build();
    footer_box.append(&footer_hint);
    vbox.append(&footer_box);

    // Window Setup
    let window = ApplicationWindow::builder()
        .application(app)
        .title("Clipboard")
        .default_width(450)
        .default_height(500)
        .decorated(false)
        .child(&vbox)
        .build();

    // Dynamic Filter: filters items as the user types
    let search_text = Rc::new(RefCell::new(String::new()));
    let search_text_clone = search_text.clone();

    list_box.set_filter_func(move |row| {
        let query = search_text_clone.borrow().to_lowercase();
        if query.is_empty() {
            return true;
        }

        // Check the text stored on the row
        if let Some(content) = get_row_content(row) {
            content.to_lowercase().contains(&query)
        } else {
            true
        }
    });

    let list_box_ref = list_box.clone();
    search_entry.connect_search_changed(move |entry| {
        *search_text.borrow_mut() = entry.text().to_string();
        list_box_ref.invalidate_filter();
    });

    // Action: Copy selected item & close window
    let window_ref = window.clone();
    let history_for_activate = history.clone();
    let count_badge_ref = count_badge.clone();

    let activate_selected = move |row: &ListBoxRow| {
        if let Some(content) = get_row_content(row) {
            // Write to system clipboard
            if let Some(display) = gdk::Display::default() {
                display.clipboard().set_text(&content);
            }
            // LRU Promote inside history as well
            history_for_activate.lock().unwrap().add(content);
            window_ref.close();
        }
    };

    let activate_clone = activate_selected.clone();
    list_box.connect_row_activated(move |_, row| {
        activate_clone(row);
    });

    // Keyboard Controller: Escape (Close), Delete (Remove item), Enter (Select)
    let key_controller = EventControllerKey::new();
    let window_for_keys = window.clone();
    let history_for_keys = history.clone();
    let list_box_for_keys = list_box.clone();

    key_controller.connect_key_pressed(move |_, key, _, _| {
        match key {
            gdk::Key::Escape => {
                window_for_keys.close();
                glib::Propagation::Stop
            }
            gdk::Key::Delete => {
                if let Some(row) = list_box_for_keys.selected_row() {
                    let idx = row.index();
                    if idx >= 0 {
                        let mut hist = history_for_keys.lock().unwrap();
                        hist.remove_at(idx as usize);
                        count_badge_ref.set_label(&format!("{} items", hist.len()));
                        populate_list(&list_box_for_keys, &hist);
                    }
                }
                glib::Propagation::Stop
            }
            gdk::Key::Return | gdk::Key::KP_Enter => {
                if let Some(row) = list_box_for_keys.selected_row() {
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

    // Auto-focus search entry
    search_entry.grab_focus();

    window.present();
}

fn populate_list(list_box: &ListBox, history: &ClipboardHistory) {
    // Clear existing rows
    while let Some(child) = list_box.first_child() {
        list_box.remove(&child);
    }

    for (i, item) in history.get_items().iter().enumerate() {
        let row = ListBoxRow::builder()
            .css_classes(["history-row"])
            .build();

        let row_box = GtkBox::new(Orientation::Vertical, 2);

        // Top line: Index + Content snippet
        let top_box = GtkBox::new(Orientation::Horizontal, 6);
        let idx_label = Label::builder()
            .label(&format!("{}.", i + 1))
            .css_classes(["item-index"])
            .build();
        top_box.append(&idx_label);

        // Truncate long preview strings for clean aesthetics
        let preview = if item.content.len() > 60 {
            format!("{}...", &item.content[..60].replace('\n', " "))
        } else {
            item.content.replace('\n', " ")
        };

        let content_label = Label::builder()
            .label(&preview)
            .css_classes(["item-content"])
            .xalign(0.0)
            .hexpand(true)
            .ellipsize(gtk4::pango::EllipsizeMode::End)
            .build();
        top_box.append(&content_label);
        row_box.append(&top_box);

        // Bottom line: Relative timestamp + character count
        let meta_text = format!("{} • {} chars", item.relative_time(), item.content.len());
        let meta_label = Label::builder()
            .label(&meta_text)
            .css_classes(["item-meta"])
            .xalign(0.0)
            .build();
        row_box.append(&meta_label);

        // Store original content on the widget for filtering and selection
        unsafe {
            row.set_data("clipboard-content", item.content.clone());
        }

        row.set_child(Some(&row_box));
        list_box.append(&row);
    }

    // Select the first item by default if available
    if let Some(first_row) = list_box.row_at_index(0) {
        list_box.select_row(Some(&first_row));
    }
}

fn get_row_content(row: &ListBoxRow) -> Option<String> {
    unsafe { row.data::<String>("clipboard-content").map(|s| s.as_ref().clone()) }
}
