use std::collections::VecDeque;
use std::time::SystemTime;

#[derive(Debug, Clone)]
pub struct ClipboardItem {
    pub content: String,
    pub timestamp: SystemTime,
}

#[derive(Debug, Clone)]
pub struct ClipboardHistory {
    items: VecDeque<ClipboardItem>,
    max_capacity: usize,
}

impl ClipboardHistory {
    pub fn new(max_capacity: usize) -> Self {
        Self {
            items: VecDeque::with_capacity(max_capacity),
            max_capacity,
        }
    }

    pub fn add(&mut self, text: String) {
        let item = ClipboardItem {
            content: text,
            timestamp: SystemTime::now(),
        };

        self.items.push_front(item);
        if self.items.len() > self.max_capacity {
            self.items.pop_back();
        }
    }

    pub fn get_items(&self) -> &VecDeque<ClipboardItem> {
        return &self.items;
    }
}
