// SPDX-License-Identifier: GPL-3.0-only

use calloop::{EventLoop, channel};
use calloop_wayland_source::WaylandSource;
use std::{collections::HashMap, os::fd::AsFd, thread, time};
use wayland_client::{
    Connection, Dispatch, Proxy, QueueHandle, WEnum, delegate_noop,
    protocol::{
        wl_keyboard::WlKeyboard,
        wl_registry,
        wl_seat::{self, WlSeat},
    },
};
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::{
    zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1,
    zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1,
};
use xkbcommon::xkb;

use crate::layout::Layout;

pub use xkb::Keycode;

#[derive(Clone, Copy, Debug)]
pub enum VkEvent {
    Key(Keycode, bool),
}

pub fn vk_channels() -> (channel::Sender<VkEvent>, channel::Channel<Layout>) {
    let (vke_tx, vke_rx) = channel::channel();
    let (layout_tx, layout_rx) = channel::channel();

    //TODO: get errors from thread?
    thread::spawn(move || {
        let mut event_loop: EventLoop<State> = EventLoop::try_new().unwrap();
        let loop_handle = event_loop.handle();

        let timer = time::Instant::now();
        loop_handle
            .insert_source(vke_rx, move |event, _metadata, state| {
                eprintln!("{:?}", event);
                let channel::Event::Msg(vke) = event else {
                    return;
                };
                //TODO: retry keys once seat and vk are available?
                //TODO: which seat should be used?
                for (_id, seat) in state.seats.iter_mut() {
                    let Some(vk) = &seat.vk else {
                        continue;
                    };
                    let Some(state) = &mut seat.state else {
                        continue;
                    };
                    //TODO: What happens on time rollover?
                    let time = timer.elapsed().as_millis() as u32;
                    match vke {
                        VkEvent::Key(kc, pressed) => {
                            let comps = state.update_key(
                                kc,
                                if pressed {
                                    xkb::KeyDirection::Down
                                } else {
                                    xkb::KeyDirection::Up
                                },
                            );
                            //TODO: check comps bits
                            if comps & xkb::STATE_MODS_EFFECTIVE > 0 {
                                let mods = state.serialize_mods(comps);
                                println!("{:#x}: {:#x}", comps, mods);
                                vk.modifiers(mods, 0, 0, 0);
                            }
                            vk.key(
                                time,
                                u32::from(kc.raw().checked_sub(8).unwrap()),
                                if pressed { 1 } else { 0 },
                            );
                        }
                    }
                    return;
                }
                eprintln!("no seat with virtual keyboard found");
            })
            .unwrap();

        let conn = Connection::connect_to_env().unwrap();

        let event_queue = conn.new_event_queue();
        let qh = event_queue.handle();

        let display = conn.display();
        display.get_registry(&qh, ());

        WaylandSource::new(conn, event_queue)
            .insert(loop_handle)
            .unwrap();

        let mut state = State {
            layout_tx,
            seats: HashMap::new(),
            vkm: None,
            xkb_ctx: xkb::Context::new(0),
        };
        while let Ok(_) = event_loop.dispatch(None, &mut state) {}
    });

    (vke_tx, layout_rx)
}

struct Seat {
    wl: WlSeat,
    keyboard: Option<WlKeyboard>,
    state: Option<xkb::State>,
    vk: Option<ZwpVirtualKeyboardV1>,
}

struct State {
    layout_tx: channel::Sender<Layout>,
    seats: HashMap<u32, Seat>,
    vkm: Option<ZwpVirtualKeyboardManagerV1>,
    xkb_ctx: xkb::Context,
}

impl Dispatch<wl_registry::WlRegistry, ()> for State {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            if interface == WlSeat::interface().name {
                eprintln!("Seat");
                state.seats.insert(
                    name,
                    Seat {
                        wl: registry.bind(name, version, qh, name),
                        keyboard: None,
                        state: None,
                        vk: None,
                    },
                );
            } else if interface == ZwpVirtualKeyboardManagerV1::interface().name {
                eprintln!("Virtual Keyboard Interface");
                assert!(state.vkm.is_none());
                state.vkm = Some(registry.bind(name, version, qh, ()));
            }
        }
    }
}

impl Dispatch<WlKeyboard, u32> for State {
    fn event(
        state: &mut Self,
        _: &WlKeyboard,
        event: <WlKeyboard as Proxy>::Event,
        &seat: &u32,
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        use wayland_client::protocol::wl_keyboard::Event;

        eprintln!("Keyboard event: {event:?}");
        //TODO: why is this event called on every keypress?
        if let Event::Keymap { format, fd, size } = event {
            let Some(ref vkm) = state.vkm else {
                eprintln!("no virtual keyboard manager found");
                return;
            };
            let Some(seat) = state.seats.get_mut(&seat) else {
                eprintln!("seat {seat} not found");
                return;
            };
            if seat.vk.is_some() {
                //TODO: allow resetting if the physical keyboard's layout was reset
                eprintln!("refusing to reset virtual keyboard keymap");
                return;
            }
            let vk = seat
                .vk
                .get_or_insert_with(|| vkm.create_virtual_keyboard(&seat.wl, qh, ()));
            vk.keymap(format.into(), fd.as_fd(), size);
            match unsafe {
                xkb::Keymap::new_from_fd(
                    &state.xkb_ctx,
                    fd,
                    size.try_into().unwrap(),
                    format.into(),
                    0,
                )
            } {
                Ok(Some(keymap)) => {
                    for layout in 0..keymap.num_layouts() {
                        println!("layout {}: {}", layout, keymap.layout_get_name(layout));
                        for kc_raw in keymap.min_keycode().raw()..=keymap.max_keycode().raw() {
                            let kc = xkb::Keycode::new(kc_raw);
                            print!("  keycode {:?} {:?}:", kc, keymap.key_get_name(kc));
                            for level in 0..keymap.num_levels_for_key(kc, layout) {
                                for ks in keymap.key_get_syms_by_level(kc, layout, level) {
                                    print!(" {:?} ({:?})", ks, xkb::keysym_get_name(*ks));
                                }
                            }
                            println!();
                        }
                        // Only show first layout for now
                        break;
                    }

                    for modifier in keymap.mods() {
                        println!("mod {}", modifier);
                    }

                    state.layout_tx.send(Layout::from(&keymap)).unwrap();
                    seat.state = Some(xkb::State::new(&keymap));
                }
                Ok(None) => {
                    eprintln!("no keymap found");
                }
                Err(err) => {
                    eprintln!("failed to parse keymap: {}", err);
                }
            }
        }
    }
}

impl Dispatch<WlSeat, u32> for State {
    fn event(
        state: &mut Self,
        wl_seat: &WlSeat,
        event: <WlSeat as Proxy>::Event,
        &id: &u32,
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        use wl_seat::Event;
        eprintln!("Seat event: {event:?}");
        match event {
            Event::Capabilities { capabilities } => {
                let WEnum::Value(caps) = capabilities else {
                    eprintln!("invalid seat {id} capabilities {capabilities:?}");
                    return;
                };
                if caps.contains(wl_seat::Capability::Keyboard) {
                    eprintln!("Seat {id} keyboard");
                    let Some(seat) = state.seats.get_mut(&id) else {
                        eprintln!("failed to find seat {id}");
                        return;
                    };
                    assert!(seat.keyboard.is_none());
                    seat.keyboard = Some(wl_seat.get_keyboard(qh, id));
                }
            }
            _ => {}
        }
    }
}

delegate_noop!(State: ZwpVirtualKeyboardManagerV1);
delegate_noop!(State: ZwpVirtualKeyboardV1);
