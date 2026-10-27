use std::collections::VecDeque;
use std::time::{Duration, SystemTime};

#[derive(Debug, Clone)]
pub struct ClipboardItem {
    pub content: String,
    pub timestamp: SystemTime,
}

impl ClipboardItem {
    pub fn new(content: String) -> Self {
        Self {
            content,
            timestamp: SystemTime::now(),
        }
    }

    /// Formats the timestamp relative to now (e.g. "just now", "3m ago")
    pub fn relative_time(&self) -> String {
        let elapsed = self.timestamp.elapsed().unwrap_or(Duration::ZERO);
        let secs = elapsed.as_secs();

        if secs < 60 {
            "just now".to_string()
        } else if secs < 3600 {
            format!("{}m ago", secs / 60)
        } else if secs < 86400 {
            format!("{}h ago", secs / 3600)
        } else {
            format!("{}d ago", secs / 86400)
        }
    }
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

    /// Adds new text to history.
    /// - Ignores empty or whitespace-only text.
    /// - If the item already exists, promotes it to the front (LRU) with an updated timestamp.
    /// - Evicts the oldest item if capacity is exceeded.
    /// Returns true if an item was added or promoted.
    pub fn add(&mut self, text: String) -> bool {
        if text.trim().is_empty() {
            return false;
        }

        // Deduplication & LRU Promotion:
        // If it already exists anywhere in the history, remove it so we can push to front.
        if let Some(pos) = self.items.iter().position(|item| item.content == text) {
            self.items.remove(pos);
        }

        // Push fresh copy to the top
        self.items.push_front(ClipboardItem::new(text));

        // Enforce maximum capacity bound
        if self.items.len() > self.max_capacity {
            self.items.pop_back();
        }

        true
    }

    pub fn remove_at(&mut self, index: usize) -> Option<ClipboardItem> {
        self.items.remove(index)
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn get_items(&self) -> &VecDeque<ClipboardItem> {
        &self.items
    }

    pub fn get(&self, index: usize) -> Option<&ClipboardItem> {
        self.items.get(index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capacity_bound() {
        let mut history = ClipboardHistory::new(3);
        history.add("Item 1".to_string());
        history.add("Item 2".to_string());
        history.add("Item 3".to_string());
        assert_eq!(history.len(), 3);

        // Exceeding capacity should evict oldest ("Item 1")
        history.add("Item 4".to_string());
        assert_eq!(history.len(), 3);
        assert_eq!(history.get(0).unwrap().content, "Item 4");
        assert_eq!(history.get(1).unwrap().content, "Item 3");
        assert_eq!(history.get(2).unwrap().content, "Item 2");
    }

    #[test]
    fn test_deduplication_and_lru_promotion() {
        let mut history = ClipboardHistory::new(5);
        history.add("A".to_string());
        history.add("B".to_string());
        history.add("C".to_string());
        assert_eq!(history.len(), 3);
        assert_eq!(history.get(0).unwrap().content, "C");

        // Re-adding "A" should promote it to index 0 without increasing length
        history.add("A".to_string());
        assert_eq!(history.len(), 3);
        assert_eq!(history.get(0).unwrap().content, "A");
        assert_eq!(history.get(1).unwrap().content, "C");
        assert_eq!(history.get(2).unwrap().content, "B");
    }

    #[test]
    fn test_ignore_empty() {
        let mut history = ClipboardHistory::new(5);
        assert!(!history.add("   ".to_string()));
        assert_eq!(history.len(), 0);
    }

    #[test]
    fn test_remove_at() {
        let mut history = ClipboardHistory::new(5);
        history.add("A".to_string());
        history.add("B".to_string());
        let removed = history.remove_at(0);
        assert_eq!(removed.unwrap().content, "B");
        assert_eq!(history.len(), 1);
        assert_eq!(history.get(0).unwrap().content, "A");
    }
}
