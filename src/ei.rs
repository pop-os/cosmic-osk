// SPDX-License-Identifier: GPL-3.0-only

use ashpd::desktop::{
    CreateSessionOptions, PersistMode,
    remote_desktop::{
        ConnectToEISOptions, DeviceType, RemoteDesktop, SelectDevicesOptions, StartOptions,
    },
};
use cosmic::iced::futures::{self, FutureExt, StreamExt};
use enumflags2::BitFlags;
use reis::ei;
use std::os::{fd::OwnedFd, unix::net::UnixStream};

const DEVICE_TYPE_KEYBOARD: u32 = 1;

#[zbus::proxy(
    interface = "com.system76.CosmicComp.Ei",
    default_service = "com.system76.CosmicComp",
    default_path = "/com/system76/CosmicComp/Ei"
)]
trait Ei {
    async fn get_sender_socket(
        &self,
        device_types: u32,
    ) -> zbus::fdo::Result<zbus::zvariant::OwnedFd>;
}

#[derive(Debug, Clone)]
pub enum Msg {
    Connection(reis::event::Connection),
    Event(reis::event::EiEvent),
}

pub fn subscription() -> cosmic::iced::Subscription<Msg> {
    cosmic::iced::Subscription::run(ei_stream)
}

fn ei_stream() -> impl futures::stream::Stream<Item = Msg> + Send {
    async {
        let conn = open_connection().await;
        // TODO Exit process on error or end of stream?
        let (connection, events) = conn
            .handshake_tokio("cosmic-osd", ei::handshake::ContextType::Sender)
            .await
            .unwrap();
        futures::stream::once(async move { Msg::Connection(connection) })
            .chain(events.map(|x| Msg::Event(x.unwrap())))
    }
    .flatten_stream()
}

async fn dbus_connection() -> zbus::Result<zbus::Connection> {
    zbus::connection::Builder::session()?
        .name("com.system76.CosmicOSK")?
        .build()
        .await
}

async fn open_connection() -> ei::Context {
    // If `LIBEI_SOCKET` env var is set, try to use that
    if let Some(context) = ei::Context::connect_to_env().unwrap() {
        context
    } else {
        let conn = dbus_connection()
            .await
            .expect("connect to DBus session socket");

        // Connect to cosmic-comp using `com.system76.CosmicComp.Ei` directly
        if let Ok(proxy) = EiProxy::new(&conn).await
            && let Ok(socket) = proxy.get_sender_socket(DEVICE_TYPE_KEYBOARD).await
        {
            let stream = UnixStream::from(OwnedFd::from(socket));
            ei::Context::new(stream).unwrap()
        } else {
            // For other compositors, try portal
            eprintln!("Unable to find ei socket. Trying xdg desktop portal.");
            let remote_desktop = RemoteDesktop::with_connection(conn).await.unwrap();
            let session = remote_desktop
                .create_session(CreateSessionOptions::default())
                .await
                .unwrap();
            let options = SelectDevicesOptions::default()
                .set_devices(BitFlags::from(DeviceType::Keyboard))
                .set_persist_mode(PersistMode::DoNot);
            remote_desktop
                .select_devices(&session, options)
                .await
                .unwrap();
            remote_desktop
                .start(&session, None, StartOptions::default())
                .await
                .unwrap();
            let fd = remote_desktop
                .connect_to_eis(&session, ConnectToEISOptions::default())
                .await
                .unwrap();
            let stream = UnixStream::from(fd);
            ei::Context::new(stream).unwrap()
        }
    }
}
