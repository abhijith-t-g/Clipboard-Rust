use std::collections::VecDeque;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::time::{Duration, SystemTime};
use gtk4::glib;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClipboardContent {
    Text(String),
    Image {
        bytes: glib::Bytes,
        width: i32,
        height: i32,
        hash: u64,
    },
}

#[derive(Debug, Clone)]
pub struct ClipboardItem {
    pub content: ClipboardContent,
    pub timestamp: SystemTime,
}

pub fn compute_bytes_hash(data: &[u8]) -> u64 {
    let mut hasher = DefaultHasher::new();
    data.hash(&mut hasher);
    hasher.finish()
}

impl ClipboardItem {
    pub fn new_text(content: String) -> Self {
        Self {
            content: ClipboardContent::Text(content),
            timestamp: SystemTime::now(),
        }
    }

    pub fn new_image(bytes: glib::Bytes, width: i32, height: i32, hash: u64) -> Self {
        Self {
            content: ClipboardContent::Image {
                bytes,
                width,
                height,
                hash,
            },
            timestamp: SystemTime::now(),
        }
    }

    #[allow(dead_code)]
    pub fn new(content: String) -> Self {
        Self::new_text(content)
    }

    #[allow(dead_code)]
    pub fn as_text(&self) -> Option<&str> {
        match &self.content {
            ClipboardContent::Text(t) => Some(t.as_str()),
            ClipboardContent::Image { .. } => None,
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
    pub fn add_text(&mut self, text: String) -> bool {
        if text.trim().is_empty() {
            return false;
        }

        // If already top item, skip re-adding
        if let Some(front) = self.items.front() {
            if let ClipboardContent::Text(ref t) = front.content {
                if t == &text {
                    return false;
                }
            }
        }

        // Deduplication & LRU Promotion:
        if let Some(pos) = self.items.iter().position(|item| match &item.content {
            ClipboardContent::Text(t) => t == &text,
            _ => false,
        }) {
            self.items.remove(pos);
        }

        // Push fresh copy to the top
        self.items.push_front(ClipboardItem::new_text(text));

        // Enforce maximum capacity bound
        if self.items.len() > self.max_capacity {
            self.items.pop_back();
        }

        true
    }

    /// Adds a new image to history.
    /// - Deduplicates based on content hash and promotes to top.
    /// - Evicts oldest item if capacity is exceeded.
    pub fn add_image(&mut self, bytes: glib::Bytes, width: i32, height: i32) -> bool {
        if bytes.is_empty() || width <= 0 || height <= 0 {
            return false;
        }

        let hash = compute_bytes_hash(bytes.as_ref());

        // If already top item, skip re-adding
        if let Some(front) = self.items.front() {
            if let ClipboardContent::Image { hash: h, .. } = front.content {
                if h == hash {
                    return false;
                }
            }
        }

        // Deduplication & LRU Promotion:
        if let Some(pos) = self.items.iter().position(|item| match &item.content {
            ClipboardContent::Image { hash: h, .. } => *h == hash,
            _ => false,
        }) {
            self.items.remove(pos);
        }

        self.items.push_front(ClipboardItem::new_image(bytes, width, height, hash));

        if self.items.len() > self.max_capacity {
            self.items.pop_back();
        }

        true
    }

    /// Backwards compatibility alias for add_text
    #[allow(dead_code)]
    pub fn add(&mut self, text: String) -> bool {
        self.add_text(text)
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
        assert_eq!(history.get(0).unwrap().as_text(), Some("Item 4"));
        assert_eq!(history.get(1).unwrap().as_text(), Some("Item 3"));
        assert_eq!(history.get(2).unwrap().as_text(), Some("Item 2"));
    }

    #[test]
    fn test_deduplication_and_lru_promotion() {
        let mut history = ClipboardHistory::new(5);
        history.add("A".to_string());
        history.add("B".to_string());
        history.add("C".to_string());
        assert_eq!(history.len(), 3);
        assert_eq!(history.get(0).unwrap().as_text(), Some("C"));

        // Re-adding "A" should promote it to index 0 without increasing length
        history.add("A".to_string());
        assert_eq!(history.len(), 3);
        assert_eq!(history.get(0).unwrap().as_text(), Some("A"));
        assert_eq!(history.get(1).unwrap().as_text(), Some("C"));
        assert_eq!(history.get(2).unwrap().as_text(), Some("B"));
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
        assert_eq!(removed.unwrap().as_text(), Some("B"));
        assert_eq!(history.len(), 1);
        assert_eq!(history.get(0).unwrap().as_text(), Some("A"));
    }

    #[test]
    fn test_image_history() {
        let mut history = ClipboardHistory::new(3);
        let bytes1 = glib::Bytes::from(&[1, 2, 3, 4]);
        let bytes2 = glib::Bytes::from(&[5, 6, 7, 8]);

        assert!(history.add_image(bytes1.clone(), 100, 100));
        assert!(history.add_image(bytes2.clone(), 200, 200));
        assert_eq!(history.len(), 2);

        // Deduplication & promotion
        assert!(history.add_image(bytes1.clone(), 100, 100));
        assert_eq!(history.len(), 2);
        match &history.get(0).unwrap().content {
            ClipboardContent::Image { width, .. } => assert_eq!(*width, 100),
            _ => panic!("Expected image at index 0"),
        }
    }
}

