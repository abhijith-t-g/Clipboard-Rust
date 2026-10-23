use std::collections::VecDeque;
use std::time::SystemTime;

#[derive(Debug,Clone)]
pub struct ClipboardItem {
    pub content: String,
    pub timestamp: SystemTime,
}

pub struct ClipboardHistory {
    items: VecDeque<ClipboardItem>,
    max_capacity: usize,
}

pub fn history() {
    println!("History Module loaded successfully");
}
