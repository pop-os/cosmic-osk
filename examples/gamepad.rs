fn main() {
    use gilrs::{Button, Event, Gilrs};

    let mut gilrs = Gilrs::new().unwrap();

    // Iterate over all connected gamepads
    for (_id, gamepad) in gilrs.gamepads() {
        println!("{} is {:?}", gamepad.name(), gamepad.power_info());
    }

    loop {
        // Examine new events
        while let Some(Event {
            id, event, time, ..
        }) = gilrs.next_event_blocking(None)
        {
            println!("{:?} New event from {}: {:?}", time, id, event);
        }
    }
}
