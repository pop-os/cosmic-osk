// SPDX-License-Identifier: GPL-3.0-only

use cosmic_osk::{
    Message,
    wayland::{VkEvent, vk_channels},
};
use std::{thread, time};

fn main() {
    let (vk_tx, vk_rx) = vk_channels();
    let Ok(Message::Layout(layout)) = vk_rx.recv() else {
        panic!()
    };
    thread::sleep(time::Duration::new(1, 0));
    eprintln!("Press A");
    {
        vk_tx
            .send(VkEvent::Key(*layout.get_keycode("a").unwrap(), true))
            .unwrap();
    }
    eprintln!("Sleep");
    thread::sleep(time::Duration::new(1, 0));
    eprintln!("Release A");
    {
        vk_tx
            .send(VkEvent::Key(*layout.get_keycode("a").unwrap(), false))
            .unwrap();
    }
    thread::sleep(time::Duration::new(1, 0));
}
