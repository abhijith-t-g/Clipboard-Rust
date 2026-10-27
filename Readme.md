# ClipboardApp

A lightweight, privacy-first, low-RAM clipboard manager designed for Linux Mint (Cinnamon) and X11/Linux desktops, written in Rust.

## Goals & Principles

- **Ultra-Low Memory Footprint**: Targets `< 25MB` resident RAM.
- **Privacy First (Non-Persistent by Design)**: Clipboard contents reside exclusively in volatile memory (RAM). No passwords, tokens, or private text are ever written to disk or database journals, leaving zero trace upon logout/reboot.
- **Deduplication & LRU Promotion**: Repeated copies bump existing entries to the top without wasting space.
- **Bounded Capacity**: Bounded ring buffer (default: 50 items) with $O(1)$ amortized push and eviction.
- **Modular Architecture**: Clean separation between storage, clipboard listening, IPC, and UI overlay.

---

## Architecture & Roadmap

```
┌────────────────────────────────────────────────────────┐
│                      clipboardApp                      │
│                                                        │
│  [ Phase 1: In-Memory History Engine ]                 │
│    - VecDeque Ring Buffer (Capacity: 50)               │
│    - Deduplication & Timestamp Tracking                │
│                                                        │
│  [ Phase 2: Clipboard Listener / Daemon ]              │
│    - X11 Selection Monitoring (CLIPBOARD / PRIMARY)    │
│    - Event Loop & Change Detection                     │
│                                                        │
│  [ Phase 3: Inter-Process Communication (IPC) ]        │
│    - DBus / Unix Domain Sockets                        │
│    - Global Shortcut Triggering                        │
│                                                        │
│  [ Phase 4: Native GTK4 UI Overlay ]                   │
│    - On-demand floating search popup                   │
│    - Instant fuzzy filtering                           │
└────────────────────────────────────────────────────────┘
```

- [x] **Phase 1: In-Memory Storage Engine**
  - [x] Pure Rust data structures (`VecDeque`, custom types)
  - [x] Capacity bounding & automatic eviction
  - [x] Deduplication and item promotion
  - [x] Unit tests for history operations
- [ ] **Phase 2: Linux Clipboard Polling / Listener Daemon**
- [ ] **Phase 3: IPC & Global Hotkey Handling**
- [x] **Phase 4: Lightweight GTK4 UI Overlay**
  - [x] Modern Catppuccin / Tokyo Night dark CSS styling
  - [x] Real-time search & fuzzy filtering
  - [x] Keyboard navigation (`Enter`, `Del`, `Esc`)
  - [x] System clipboard writing & auto-dismissal

---

## Getting Started

### Prerequisites

- Rust toolchain (`rustup`, `cargo`, `rustc`)
- Linux Mint / Cinnamon (or any X11/Wayland-compatible Linux desktop)

### Build & Run

```bash
# Run in development mode
cargo run

# Run unit tests
cargo test

# Build optimized release binary
cargo build --release
```

---

## License

MIT or Apache 2.0.