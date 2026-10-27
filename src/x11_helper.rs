use x11rb::connection::Connection;
use x11rb::protocol::xproto::{AtomEnum, ConnectionExt as _};
use x11rb::protocol::xtest::ConnectionExt as _;
use x11rb::wrapper::ConnectionExt as _;

/// Queries current mouse root coordinates and screen dimensions
pub fn get_mouse_and_screen() -> Option<(i32, i32, i32, i32)> {
    let (conn, screen_num) = x11rb::connect(None).ok()?;
    let screen = &conn.setup().roots[screen_num];
    let reply = conn.query_pointer(screen.root).ok()?.reply().ok()?;
    Some((
        reply.root_x as i32,
        reply.root_y as i32,
        screen.width_in_pixels as i32,
        screen.height_in_pixels as i32,
    ))
}

/// Gets the currently focused window ID before our popup steals focus.
/// Filters out our own "Clipboard" window so we never refocus ourselves!
pub fn get_active_window() -> Option<u32> {
    let (conn, screen_num) = x11rb::connect(None).ok()?;
    let screen = &conn.setup().roots[screen_num];
    let net_active_atom = conn
        .intern_atom(false, b"_NET_ACTIVE_WINDOW")
        .ok()?
        .reply()
        .ok()?
        .atom;
    let prop = conn
        .get_property(false, screen.root, net_active_atom, AtomEnum::WINDOW, 0, 1)
        .ok()?
        .reply()
        .ok()?;
    let win = prop.value32()?.next()?;
    if win == 0 {
        return None;
    }

    // Check if the active window is our own Clipboard window
    if let Ok(name_prop) =
        conn.get_property(false, win, AtomEnum::WM_NAME, AtomEnum::STRING, 0, 50)
    {
        if let Ok(reply) = name_prop.reply() {
            let name = String::from_utf8_lossy(&reply.value);
            if name.contains("Clipboard") {
                return None;
            }
        }
    }

    Some(win)
}

/// Computes position near cursor, clamped to screen bounds
pub fn compute_cursor_position(win_width: i32, win_height: i32) -> Option<(i32, i32)> {
    let (mouse_x, mouse_y, screen_w, screen_h) = get_mouse_and_screen()?;
    let mut target_x = mouse_x + 10;
    let mut target_y = mouse_y + 10;

    if target_x + win_width > screen_w {
        target_x = (mouse_x - win_width - 10).max(10);
    }
    if target_y + win_height > screen_h {
        target_y = (mouse_y - win_height - 10).max(10);
    }

    Some((target_x, target_y))
}

/// Sets ICCCM WM_NORMAL_HINTS with USPosition flag so the window manager
/// places the window at (x, y) immediately on map, without defaulting to (0, 0)
pub fn set_wm_normal_hints_position(xid: u32, x: i32, y: i32, w: i32, h: i32) {
    let Ok((conn, _)) = x11rb::connect(None) else {
        return;
    };
    // USPosition (1 << 0) | PPosition (1 << 2) | USSize (1 << 1) | PSize (1 << 3)
    let flags: u32 = (1 << 0) | (1 << 2) | (1 << 1) | (1 << 3);
    let hints: [u32; 18] = [
        flags,
        x as u32,
        y as u32,
        w as u32,
        h as u32,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ];
    let _ = conn.change_property32(
        x11rb::protocol::xproto::PropMode::REPLACE,
        xid,
        AtomEnum::WM_NORMAL_HINTS,
        AtomEnum::WM_SIZE_HINTS,
        &hints,
    );
    let _ = conn.flush();
}

/// Moves window offscreen when hidden so it never flashes at (0, 0) when re-mapped
pub fn move_offscreen(xid: u32) {
    let hex_id = format!("0x{:08x}", xid);
    let _ = std::process::Command::new("wmctrl")
        .args([
            "-i",
            "-r",
            &hex_id,
            "-e",
            "0,-3000,-3000,330,420",
        ])
        .status();
}

/// Positions window directly by XID before presenting
pub fn position_window_by_id(xid: u32, win_width: i32, win_height: i32) {
    if let Some((target_x, target_y)) = compute_cursor_position(win_width, win_height) {
        set_wm_normal_hints_position(xid, target_x, target_y, win_width, win_height);

        // Direct in-process EWMH client message to window manager
        if let Ok((conn, screen_num)) = x11rb::connect(None) {
            let screen = &conn.setup().roots[screen_num];
            // ConfigureWindow
            let aux = x11rb::protocol::xproto::ConfigureWindowAux::new()
                .x(target_x)
                .y(target_y)
                .width(win_width as u32)
                .height(win_height as u32);
            let _ = conn.configure_window(xid, &aux);

            // _NET_MOVERESIZE_WINDOW (Gravity: 1 NorthWest, Flags: 0x0f << 8)
            if let Ok(atom_reply) = conn.intern_atom(false, b"_NET_MOVERESIZE_WINDOW") {
                if let Ok(atom) = atom_reply.reply() {
                    let flags: u32 = 1 | (0x0f << 8);
                    let data = [
                        flags,
                        target_x as u32,
                        target_y as u32,
                        win_width as u32,
                        win_height as u32,
                    ];
                    let event = x11rb::protocol::xproto::ClientMessageEvent {
                        response_type: x11rb::protocol::xproto::CLIENT_MESSAGE_EVENT,
                        format: 32,
                        sequence: 0,
                        window: xid,
                        type_: atom.atom,
                        data: x11rb::protocol::xproto::ClientMessageData::from(data),
                    };
                    let _ = conn.send_event(
                        false,
                        screen.root,
                        x11rb::protocol::xproto::EventMask::SUBSTRUCTURE_REDIRECT
                            | x11rb::protocol::xproto::EventMask::SUBSTRUCTURE_NOTIFY,
                        event,
                    );
                }
            }
            let _ = conn.flush();
        }

        // Fast wmctrl with Gravity 1 (NorthWest / exact top-left)
        let hex_id = format!("0x{:08x}", xid);
        let _ = std::process::Command::new("wmctrl")
            .args([
                "-i",
                "-r",
                &hex_id,
                "-e",
                &format!("1,{},{},{},{}", target_x, target_y, win_width, win_height),
            ])
            .status();
    }
}

/// Fallback positioning by window title "Clipboard"
pub fn position_window_by_title(win_width: i32, win_height: i32) {
    if let Some((target_x, target_y)) = compute_cursor_position(win_width, win_height) {
        let _ = std::process::Command::new("wmctrl")
            .args([
                "-r",
                "Clipboard",
                "-e",
                &format!("1,{},{},{},{}", target_x, target_y, win_width, win_height),
            ])
            .status();
    }
}

/// Refocuses the target window and simulates Ctrl+V to paste the copied content
pub fn paste_into_window(target_window_id: Option<u32>) {
    std::thread::spawn(move || {
        // 1. Refocus target application if an external window was active
        if let Some(win_id) = target_window_id {
            let hex_id = format!("0x{:08x}", win_id);
            let _ = std::process::Command::new("wmctrl")
                .args(["-i", "-a", &hex_id])
                .status();
        }

        // Wait 150ms for target window to regain input focus
        std::thread::sleep(std::time::Duration::from_millis(150));

        let Ok((conn, _)) = x11rb::connect(None) else {
            return;
        };
        let setup = conn.setup();
        let min_keycode = setup.min_keycode;
        let max_keycode = setup.max_keycode;

        let Ok(mapping) = conn.get_keyboard_mapping(min_keycode, max_keycode - min_keycode + 1)
        else {
            return;
        };
        let Ok(mapping_reply) = mapping.reply() else {
            return;
        };
        let keysyms_per_keycode = mapping_reply.keysyms_per_keycode as usize;

        let mut ctrl_code = 37u8;
        let mut v_code = 55u8;
        let mut super_keys = Vec::new();
        let mut alt_keys = Vec::new();

        for (idx, chunk) in mapping_reply.keysyms.chunks(keysyms_per_keycode).enumerate() {
            let keycode = min_keycode + idx as u8;
            for &sym in chunk {
                match sym {
                    0xffe3 => ctrl_code = keycode,                // XK_Control_L
                    0x0076 | 0x0056 => v_code = keycode,         // XK_v / XK_V
                    0xffeb | 0xffec => super_keys.push(keycode), // XK_Super_L / XK_Super_R
                    0xffe9 | 0xffea => alt_keys.push(keycode),   // XK_Alt_L / XK_Alt_R
                    _ => {}
                }
            }
        }

        // Release any active Super/Windows or Alt keys left over from the Win+V shortcut!
        for &k in &super_keys {
            let _ = conn.xtest_fake_input(3, k, 0, 0, 0, 0, 0); // KeyRelease
        }
        for &k in &alt_keys {
            let _ = conn.xtest_fake_input(3, k, 0, 0, 0, 0, 0); // KeyRelease
        }
        let _ = conn.flush();
        std::thread::sleep(std::time::Duration::from_millis(30));

        // Send KeyPress Ctrl
        let _ = conn.xtest_fake_input(2, ctrl_code, 0, 0, 0, 0, 0);
        std::thread::sleep(std::time::Duration::from_millis(20));

        // Send KeyPress V
        let _ = conn.xtest_fake_input(2, v_code, 0, 0, 0, 0, 0);
        std::thread::sleep(std::time::Duration::from_millis(20));

        // Send KeyRelease V
        let _ = conn.xtest_fake_input(3, v_code, 0, 0, 0, 0, 0);
        std::thread::sleep(std::time::Duration::from_millis(20));

        // Send KeyRelease Ctrl
        let _ = conn.xtest_fake_input(3, ctrl_code, 0, 0, 0, 0, 0);

        let _ = conn.flush();
        // Give X server time to deliver events before connection closes
        std::thread::sleep(std::time::Duration::from_millis(100));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xtest_support() {
        let (conn, _) = x11rb::connect(None).unwrap();
        let version = conn.xtest_get_version(2, 2).unwrap().reply().unwrap();
        println!("XTest version: {}.{}", version.major_version, version.minor_version);
        assert!(version.major_version >= 2);
    }

    #[test]
    fn test_fake_input_check() {
        let (conn, _) = x11rb::connect(None).unwrap();
        let res1 = conn.xtest_fake_input(2, 37, 0, 0, 0, 0, 0).unwrap().check();
        let res2 = conn.xtest_fake_input(3, 37, 0, 0, 0, 0, 0).unwrap().check();
        println!("fake input check: res1={:?}, res2={:?}", res1, res2);
        assert!(res1.is_ok());
        assert!(res2.is_ok());
    }
}
