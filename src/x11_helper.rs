use x11rb::connection::Connection;
use x11rb::protocol::xproto::{AtomEnum, ConnectionExt as _};
use x11rb::protocol::xtest::ConnectionExt as _;

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

/// Gets the currently focused window ID before our popup steals focus
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
    prop.value32()?.next()
}

/// Positions the clipboard window right next to the cursor via EWMH/wmctrl
pub fn position_window_at_cursor(win_width: i32, win_height: i32) {
    if let Some((mouse_x, mouse_y, screen_w, screen_h)) = get_mouse_and_screen() {
        let mut target_x = mouse_x + 10;
        let mut target_y = mouse_y + 10;

        if target_x + win_width > screen_w {
            target_x = (mouse_x - win_width - 10).max(10);
        }
        if target_y + win_height > screen_h {
            target_y = (mouse_y - win_height - 10).max(10);
        }

        let _ = std::process::Command::new("wmctrl")
            .args([
                "-r",
                "Clipboard",
                "-e",
                &format!("0,{},{},{},{}", target_x, target_y, win_width, win_height),
            ])
            .status();
    }
}

/// Refocuses the target window and simulates Ctrl+V to paste the copied content
pub fn paste_into_window(target_window_id: Option<u32>) {
    std::thread::spawn(move || {
        // 1. Refocus the target application that had focus before clipboard opened
        if let Some(win_id) = target_window_id {
            let hex_id = format!("0x{:x}", win_id);
            let _ = std::process::Command::new("wmctrl")
                .args(["-i", "-a", &hex_id])
                .status();
        }

        // Wait a short moment for X11 focus switch
        std::thread::sleep(std::time::Duration::from_millis(100));

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

        for (idx, chunk) in mapping_reply.keysyms.chunks(keysyms_per_keycode).enumerate() {
            let keycode = min_keycode + idx as u8;
            for &sym in chunk {
                if sym == 0xffe3 {
                    // XK_Control_L
                    ctrl_code = keycode;
                } else if sym == 0x0076 || sym == 0x0056 {
                    // XK_v / XK_V
                    v_code = keycode;
                }
            }
        }

        // Send KeyPress Control_L
        let _ = conn.xtest_fake_input(2, ctrl_code, 0, 0, 0, 0, 0);
        // Send KeyPress 'v'
        let _ = conn.xtest_fake_input(2, v_code, 0, 0, 0, 0, 0);
        // Send KeyRelease 'v'
        let _ = conn.xtest_fake_input(3, v_code, 0, 0, 0, 0, 0);
        // Send KeyRelease Control_L
        let _ = conn.xtest_fake_input(3, ctrl_code, 0, 0, 0, 0, 0);

        let _ = conn.flush();
    });
}
