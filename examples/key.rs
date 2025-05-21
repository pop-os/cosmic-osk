// SPDX-License-Identifier: GPL-3.0-only

use cosmic_osk::wayland::{KeyCode, VkEvent, wayland_state};
use std::{thread, time};

fn main() {
    let state_wrapper = wayland_state();
    thread::sleep(time::Duration::new(1, 0));
    eprintln!("Press A");
    {
        let state = state_wrapper.0.read().unwrap();
        state.vk_event(VkEvent::KeyPress(KeyCode::KEY_A));
    }
    eprintln!("Sleep");
    thread::sleep(time::Duration::new(1, 0));
    eprintln!("Release A");
    {
        let state = state_wrapper.0.read().unwrap();
        state.vk_event(VkEvent::KeyPress(KeyCode::KEY_A));
    }
    thread::sleep(time::Duration::new(1, 0));
}
