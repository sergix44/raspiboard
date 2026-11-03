//! Native Wayland window icons using xdg-toplevel-icon-v1.

use sctk::globals::GlobalData;
use sctk::reexports::client::globals::{BindError, GlobalList};
use sctk::reexports::client::protocol::wl_shm::Format;
use sctk::reexports::client::{delegate_dispatch, Connection, Dispatch, Proxy, QueueHandle};
use sctk::reexports::protocols::xdg::shell::client::xdg_toplevel::XdgToplevel;
use sctk::shm::slot::{Buffer, CreateBufferError, SlotPool};
use wayland_protocols::xdg::toplevel_icon::v1::client::xdg_toplevel_icon_manager_v1::XdgToplevelIconManagerV1;
use wayland_protocols::xdg::toplevel_icon::v1::client::xdg_toplevel_icon_v1::XdgToplevelIconV1;

use super::icon_pixels::{icon_layout, write_argb8888};
use crate::icon::RgbaIcon;
use crate::platform_impl::wayland::state::WinitState;

#[derive(Clone, Debug)]
pub struct XdgToplevelIconManagerState {
    manager: XdgToplevelIconManagerV1,
}

#[derive(Debug)]
pub enum ToplevelIconError {
    InvalidDimensions,
    Shm(CreateBufferError),
}

impl std::fmt::Display for ToplevelIconError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidDimensions => {
                f.write_str("icon dimensions exceed Wayland shared-memory limits")
            },
            Self::Shm(error) => error.fmt(f),
        }
    }
}

#[derive(Debug)]
pub struct ToplevelIcon {
    icon: XdgToplevelIconV1,
    // The protocol does not use wl_buffer.release: keep both buffer and its slot
    // immutable until after destroying the associated protocol icon.
    _buffer: Buffer,
}

impl Drop for ToplevelIcon {
    fn drop(&mut self) {
        self.icon.destroy();
    }
}

impl XdgToplevelIconManagerState {
    pub fn bind(globals: &GlobalList, queue: &QueueHandle<WinitState>) -> Result<Self, BindError> {
        Ok(Self { manager: globals.bind(queue, 1..=1, GlobalData)? })
    }

    pub(crate) fn set_icon(
        &self,
        toplevel: &XdgToplevel,
        icon: Option<RgbaIcon>,
        pool: &mut SlotPool,
        queue: &QueueHandle<WinitState>,
    ) -> Result<Option<ToplevelIcon>, ToplevelIconError> {
        let icon = match icon {
            Some(image) => {
                let layout = icon_layout(image.width, image.height)
                    .ok_or(ToplevelIconError::InvalidDimensions)?;
                let (buffer, canvas) = pool
                    .create_buffer(layout.edge, layout.edge, layout.stride, Format::Argb8888)
                    .map_err(ToplevelIconError::Shm)?;
                write_argb8888(&image.rgba, image.width as usize, image.height as usize, canvas);
                let icon = self.manager.create_icon(queue, GlobalData);
                icon.add_buffer(buffer.wl_buffer(), 1);
                Some(ToplevelIcon { icon, _buffer: buffer })
            },
            None => None,
        };
        self.manager.set_icon(toplevel, icon.as_ref().map(|icon| &icon.icon));
        Ok(icon)
    }
}

impl Dispatch<XdgToplevelIconManagerV1, GlobalData, WinitState> for XdgToplevelIconManagerState {
    fn event(
        _: &mut WinitState,
        _: &XdgToplevelIconManagerV1,
        _: <XdgToplevelIconManagerV1 as Proxy>::Event,
        _: &GlobalData,
        _: &Connection,
        _: &QueueHandle<WinitState>,
    ) {
        // Size preferences are optional; use the size supplied by the application.
    }
}

impl Dispatch<XdgToplevelIconV1, GlobalData, WinitState> for XdgToplevelIconManagerState {
    fn event(
        _: &mut WinitState,
        _: &XdgToplevelIconV1,
        _: <XdgToplevelIconV1 as Proxy>::Event,
        _: &GlobalData,
        _: &Connection,
        _: &QueueHandle<WinitState>,
    ) {
    }
}

delegate_dispatch!(WinitState: [XdgToplevelIconManagerV1: GlobalData] => XdgToplevelIconManagerState);
delegate_dispatch!(WinitState: [XdgToplevelIconV1: GlobalData] => XdgToplevelIconManagerState);

#[cfg(test)]
mod tests {
    use std::io::Read;
    use std::os::fd::OwnedFd;
    use std::os::unix::net::UnixStream;
    use std::sync::Arc;

    use sctk::reexports::client::backend::{Backend, ObjectData, ObjectId};
    use sctk::reexports::client::protocol::{wl_registry, wl_shm::WlShm};
    use sctk::shm::Shm;
    use wayland_backend::protocol::Message;

    use super::*;

    // Only inspect outbound requests; no compositor, window, or event loop is run.
    struct NoEvents;

    impl ObjectData for NoEvents {
        fn event(
            self: Arc<Self>,
            _: &Backend,
            _: Message<ObjectId, OwnedFd>,
        ) -> Option<Arc<dyn ObjectData>> {
            panic!("the request-only peer must not send events");
        }
        fn destroyed(&self, _: ObjectId) {}
    }

    fn bind<I: Proxy>(registry: &wl_registry::WlRegistry, name: u32) -> I {
        registry
            .send_constructor(
                wl_registry::Request::Bind { name, id: (I::interface(), 1) },
                Arc::new(NoEvents),
            )
            .unwrap()
    }

    fn requests(connection: &Connection, peer: &mut UnixStream) -> Vec<(u32, u16, Vec<u32>)> {
        connection.flush().unwrap();
        let mut bytes = Vec::new();
        let mut buffer = [0; 4096];
        loop {
            match peer.read(&mut buffer) {
                Ok(0) => panic!("connection unexpectedly closed"),
                Ok(count) => bytes.extend_from_slice(&buffer[..count]),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(error) => panic!("{error}"),
            }
        }
        let mut result = Vec::new();
        let mut offset = 0;
        while offset < bytes.len() {
            let word = |at| u32::from_ne_bytes(bytes[at..at + 4].try_into().unwrap());
            let header = word(offset + 4);
            let len = (header >> 16) as usize;
            assert!(len >= 8 && offset + len <= bytes.len());
            result.push((
                word(offset),
                header as u16,
                (offset + 8..offset + len).step_by(4).map(word).collect(),
            ));
            offset += len;
        }
        result
    }

    fn image() -> RgbaIcon {
        RgbaIcon::from_rgba(vec![255, 128, 64, 128], 1, 1).unwrap()
    }

    #[test]
    fn setting_replacing_and_clearing_icons_keeps_buffers_alive_until_icon_destruction() {
        let (client, mut peer) = UnixStream::pair().unwrap();
        peer.set_nonblocking(true).unwrap();
        let connection = Connection::from_socket(client).unwrap();
        let queue = connection.new_event_queue::<WinitState>();
        let handle = queue.handle();
        let registry = connection
            .display()
            .send_constructor::<wl_registry::WlRegistry>(
                sctk::reexports::client::protocol::wl_display::Request::GetRegistry {},
                Arc::new(NoEvents),
            )
            .unwrap();
        let shm: WlShm = bind(&registry, 1);
        let manager = XdgToplevelIconManagerState { manager: bind(&registry, 2) };
        let toplevel: XdgToplevel = bind(&registry, 3);
        let mut pool = SlotPool::new(128, &Shm::from(shm)).unwrap();
        requests(&connection, &mut peer);

        let mut current = manager.set_icon(&toplevel, Some(image()), &mut pool, &handle).unwrap();
        let first = current.as_ref().unwrap();
        let first_icon = first.icon.id().protocol_id();
        let first_buffer = first._buffer.wl_buffer().id().protocol_id();
        assert_eq!(first._buffer.canvas(&mut pool).unwrap(), &0x80804020_u32.to_ne_bytes());
        let sent = requests(&connection, &mut peer);
        assert_eq!(sent.len(), 4);
        assert_eq!(sent[0].1, 0); // wl_shm_pool.create_buffer
        assert_eq!(sent[0].2[0], first_buffer);
        assert_eq!(sent[0].2[2..], [1, 1, 4, 0]); // width, height, stride, ARGB8888
        assert_eq!(sent[1], (manager.manager.id().protocol_id(), 1, vec![first_icon]));
        assert_eq!(sent[2], (first_icon, 2, vec![first_buffer, 1]));
        assert_eq!(
            sent[3],
            (manager.manager.id().protocol_id(), 2, vec![toplevel.id().protocol_id(), first_icon])
        );

        let replacement = manager
            .set_icon(
                &toplevel,
                Some(RgbaIcon::from_rgba(vec![0, 255, 0, 255], 1, 1).unwrap()),
                &mut pool,
                &handle,
            )
            .unwrap();
        // Allocating a replacement must not overwrite storage still owned by the old icon.
        assert_eq!(
            current.as_ref().unwrap()._buffer.canvas(&mut pool).unwrap(),
            &0x80804020_u32.to_ne_bytes()
        );
        let next_icon = replacement.as_ref().unwrap().icon.id().protocol_id();
        let next_buffer = replacement.as_ref().unwrap()._buffer.wl_buffer().id().protocol_id();
        current = replacement;
        let sent = requests(&connection, &mut peer);
        assert_eq!(sent.len(), 6);
        assert_eq!(
            sent[3],
            (manager.manager.id().protocol_id(), 2, vec![toplevel.id().protocol_id(), next_icon])
        );
        assert_eq!(sent[4], (first_icon, 0, vec![]));
        assert_eq!(sent[5], (first_buffer, 0, vec![]));

        let cleared = manager.set_icon(&toplevel, None, &mut pool, &handle).unwrap();
        assert!(cleared.is_none());
        drop(current);
        assert_eq!(
            requests(&connection, &mut peer),
            [
                (manager.manager.id().protocol_id(), 2, vec![toplevel.id().protocol_id(), 0]),
                (next_icon, 0, vec![]),
                (next_buffer, 0, vec![]),
            ]
        );
    }
}
