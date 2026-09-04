// SPDX-License-Identifier: GPL-3.0-only

use calloop::EventLoop;
use calloop_wayland_source::WaylandSource;
use cosmic::iced::futures::{self, SinkExt};
use std::collections::HashMap;
use wayland_client::{
    Connection, Dispatch, Proxy, QueueHandle, WEnum, delegate_noop,
    protocol::{
        wl_registry,
        wl_seat::{self, WlSeat},
    },
};
use wayland_protocols_misc::zwp_input_method_v2::client::{
    zwp_input_method_manager_v2::ZwpInputMethodManagerV2,
    zwp_input_method_v2::{self, ZwpInputMethodV2},
};

use crate::Message;

pub fn wayland_task(msg_tx: futures::channel::mpsc::Sender<Message>) {
    //TODO: get errors from thread?
    let mut event_loop: EventLoop<State> = EventLoop::try_new().unwrap();
    let loop_handle = event_loop.handle();

    let conn = Connection::connect_to_env().unwrap();

    let event_queue = conn.new_event_queue();
    let qh = event_queue.handle();

    let display = conn.display();
    display.get_registry(&qh, ());

    WaylandSource::new(conn, event_queue)
        .insert(loop_handle)
        .unwrap();

    let mut state = State {
        msg_tx,
        seats: HashMap::new(),
        imm: None,
    };
    while let Ok(_) = event_loop.dispatch(None, &mut state) {}
}

struct Seat {
    wl: WlSeat,
    im: Option<ZwpInputMethodV2>,
    im_active: bool,
}

struct State {
    msg_tx: futures::channel::mpsc::Sender<Message>,
    seats: HashMap<u32, Seat>,
    imm: Option<ZwpInputMethodManagerV2>,
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
                log::info!("Seat");
                state.seats.insert(
                    name,
                    Seat {
                        wl: registry.bind(name, version, qh, name),
                        im: None,
                        im_active: false,
                    },
                );
            } else if interface == ZwpInputMethodManagerV2::interface().name {
                log::info!("Input Method Interface");
                assert!(state.imm.is_none());
                state.imm = Some(registry.bind(name, version, qh, ()));
            }
        }
    }
}

impl Dispatch<WlSeat, u32> for State {
    fn event(
        state: &mut Self,
        wl_seat: &WlSeat,
        event: <WlSeat as Proxy>::Event,
        &seat_id: &u32,
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        use wl_seat::Event;
        log::info!("Seat {seat_id} event: {event:?}");
        match event {
            Event::Capabilities { capabilities } => {
                let WEnum::Value(caps) = capabilities else {
                    log::info!("invalid seat {seat_id} capabilities {capabilities:?}");
                    return;
                };
                if caps.contains(wl_seat::Capability::Keyboard) {
                    log::info!("Seat {seat_id} keyboard");
                    let Some(seat) = state.seats.get_mut(&seat_id) else {
                        log::info!("failed to find seat {seat_id}");
                        return;
                    };

                    if let Some(ref imm) = state.imm {
                        seat.im
                            .get_or_insert_with(|| imm.get_input_method(&seat.wl, qh, seat_id));
                    } else {
                        log::info!("no input method manager found");
                    }
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<ZwpInputMethodV2, u32> for State {
    fn event(
        state: &mut Self,
        im: &ZwpInputMethodV2,
        event: zwp_input_method_v2::Event,
        &seat_id: &u32,
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        use zwp_input_method_v2::Event;
        let Some(seat) = state.seats.get_mut(&seat_id) else {
            log::info!("seat {seat_id} not found");
            return;
        };
        log::info!("seat {} input method: {:?}", seat_id, event);
        match event {
            Event::Activate => {
                seat.im_active = true;
            }
            Event::Deactivate => {
                seat.im_active = false;
            }
            Event::Done => futures::executor::block_on(async {
                state
                    .msg_tx
                    .send(Message::SeatImActive {
                        seat_id,
                        active: seat.im_active,
                    })
                    .await
            })
            .expect("failed to send seat active event"),
            //TODO: handle more events
            _ => {}
        }
    }
}

delegate_noop!(State: ZwpInputMethodManagerV2);
