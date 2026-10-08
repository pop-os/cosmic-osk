// SPDX-License-Identifier: GPL-3.0-only

use ashpd::desktop::{
    CreateSessionOptions, PersistMode,
    remote_desktop::{
        ConnectToEISOptions, DeviceType, RemoteDesktop, SelectDevicesOptions, StartOptions,
    },
};
use cosmic::iced::futures::{self, FutureExt, StreamExt};
use cosmic::{
    action::{self, Action},
    iced::futures::stream::SelectAll,
};
use enumflags2::BitFlags;
use reis::ei;
use std::os::{fd::OwnedFd, unix::net::UnixStream};
use wayland_protocols::wp::text_input::zv3::client::zwp_text_input_v3::{
    ContentHint, ContentPurpose,
};

use crate::Message;

const DEVICE_TYPE_KEYBOARD: u32 = 1;
const DEVICE_TYPE_POINTER: u32 = 2;

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

    #[zbus(signal)]
    async fn activated(&self, content_hint: u32, content_purpose: u32) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn deactivated(&self) -> zbus::Result<()>;
}

#[derive(Debug, Clone)]
pub enum Msg {
    Connection(reis::event::Connection),
    Event(reis::event::EiEvent),
}

pub fn stream(
    zbus: zbus::Connection,
) -> impl futures::stream::Stream<Item = Action<Message>> + Send {
    async {
        let mut select_all = SelectAll::new();
        let conn = open_connection(zbus, &mut select_all).await;
        // TODO Exit process on error or end of stream?
        let (connection, events) = conn
            .handshake_tokio("cosmic-osk", ei::handshake::ContextType::Sender)
            .await
            .unwrap();
        select_all.push(Box::new(
            events.map(|x| action::app(Message::Ei(Msg::Event(x.unwrap())))),
        ));
        futures::stream::once(async move { action::app(Message::Ei(Msg::Connection(connection))) })
            .chain(select_all)
    }
    .flatten_stream()
}

async fn open_connection(
    zbus: zbus::Connection,
    select_all: &mut SelectAll<
        Box<dyn futures::stream::Stream<Item = Action<Message>> + Send + Unpin>,
    >,
) -> ei::Context {
    // If `LIBEI_SOCKET` env var is set, try to use that
    if let Some(context) = ei::Context::connect_to_env().unwrap() {
        context
    } else {
        // Connect to cosmic-comp using `com.system76.CosmicComp.Ei` directly
        if let Ok(proxy) = EiProxy::new(&zbus).await
            && let Ok(socket) = proxy
                .get_sender_socket(DEVICE_TYPE_KEYBOARD | DEVICE_TYPE_POINTER)
                .await
        {
            match proxy.receive_activated().await {
                Ok(stream) => select_all.push(Box::new(stream.map(|x| {
                    // TODO: use content hint and purpose
                    let (_content_hint, _content_purpose) = match x.args() {
                        Ok(args) => (
                            match ContentHint::from_bits(args.content_hint) {
                                Some(some) => some,
                                None => {
                                    log::warn!(
                                        "failed to parse activated content hint {:#x}",
                                        args.content_hint
                                    );
                                    ContentHint::None
                                }
                            },
                            match ContentPurpose::try_from(args.content_purpose) {
                                Ok(ok) => ok,
                                Err(err) => {
                                    log::warn!(
                                        "failed to parse activated content purpose {:#x}: {:?}",
                                        args.content_purpose,
                                        err
                                    );
                                    ContentPurpose::Normal
                                }
                            },
                        ),
                        Err(err) => {
                            log::warn!("failed to parse activated args: {:?}", err);
                            (ContentHint::None, ContentPurpose::Normal)
                        }
                    };
                    action::app(Message::ImActive { active: true })
                }))),
                Err(err) => {
                    log::warn!(
                        "failed to receive activated signal from cosmic-comp: {:?}",
                        err
                    );
                }
            }

            match proxy.receive_deactivated().await {
                Ok(stream) => {
                    select_all.push(Box::new(
                        stream.map(|_| action::app(Message::ImActive { active: false })),
                    ));
                }
                Err(err) => {
                    log::warn!(
                        "failed to receive deactivated signal from cosmic-comp: {:?}",
                        err
                    );
                }
            }

            let stream = UnixStream::from(OwnedFd::from(socket));
            ei::Context::new(stream).unwrap()
        } else {
            // For other compositors, try portal
            eprintln!("Unable to find ei socket. Trying xdg desktop portal.");
            let remote_desktop = RemoteDesktop::with_connection(zbus).await.unwrap();
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
