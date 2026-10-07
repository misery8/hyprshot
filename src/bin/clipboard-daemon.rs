use std::os::fd::AsFd;
use std::{
    collections::HashMap,
    io::{Cursor, Read},
};

use anyhow::{Context, Result};
use nix::poll::{poll, PollFd, PollFlags, PollTimeout};
use nix::unistd::write;

use wayland_client::{
    protocol::{wl_data_device, wl_data_device_manager, wl_data_source, wl_registry, wl_seat},
    Connection, Dispatch, QueueHandle,
};

struct State {
    data: HashMap<String, LazyData>,
    quit: bool,
}

impl Dispatch<wl_data_source::WlDataSource, ()> for State {
    fn event(
        state: &mut Self,
        proxy: &wl_data_source::WlDataSource,
        event: wl_data_source::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        match event {
            wl_data_source::Event::Send { mime_type, fd } => {
                if let Some(buf) = state.data.get_mut(&mime_type) {
                    let buf = buf.get();
                    let fd = fd.as_fd();
                    let mut written = 0;
                    while written < buf.len() {
                        match write(fd, &buf[written..]) {
                            Ok(0) => break,
                            Ok(n) => written += n,
                            Err(nix::errno::Errno::EAGAIN) => {
                                let mut pfd = [PollFd::new(fd, PollFlags::POLLOUT)];
                                let _ = poll(&mut pfd, PollTimeout::NONE);
                            }
                            Err(_) => break,
                        }
                    }
                }
            }
            wl_data_source::Event::Cancelled => {
                proxy.destroy();
                state.quit = true;
            }
            _ => {}
        }
    }
}

wayland_client::delegate_noop!(State: ignore wl_seat::WlSeat);
wayland_client::delegate_dispatch!(State: [wl_registry::WlRegistry: wayland_client::globals::GlobalListContents] => State);
wayland_client::delegate_dispatch!(State: [wl_data_device::WlDataDevice: ()] => State);
wayland_client::delegate_dispatch!(State: [wl_data_device_manager::WlDataDeviceManager: ()] => State);

enum LazyData {
    Ready(Vec<u8>),
    Lazy(Box<dyn FnOnce() -> Vec<u8> + Send>),
}

impl LazyData {
    fn get(&mut self) -> &[u8] {
        if matches!(self, LazyData::Lazy(_)) {
            let lazy = std::mem::replace(self, LazyData::Ready(Vec::new()));
            let LazyData::Lazy(build) = lazy else {
                unreachable!();
            };
            *self = LazyData::Ready(build());
        }

        match self {
            LazyData::Ready(data) => data.as_slice(),
            LazyData::Lazy(_) => unreachable!(),
        }
    }
}

fn main() -> Result<()> {
    let mut png_data = Vec::new();
    std::io::stdin()
        .read_to_end(&mut png_data)
        .context("Failed to read stdin")?;

    let mut data_map = HashMap::new();
    data_map.insert("image/png".to_string(), LazyData::Ready(png_data.clone()));
    data_map.insert(
        "image/bmp".to_string(),
        LazyData::Lazy(Box::new(move || {
            let img = image::load_from_memory(&png_data).unwrap();
            let mut bmp = Cursor::new(Vec::with_capacity(png_data.capacity()));
            img.write_to(&mut bmp, image::ImageFormat::Bmp).unwrap();
            bmp.into_inner()
        })),
    );

    let mut state = State {
        data: data_map,
        quit: false,
    };

    let conn = Connection::connect_to_env().context("Failed to connect to Wayland")?;
    let (globals, mut event_queue) = wayland_client::globals::registry_queue_init(&conn)?;
    let qh = event_queue.handle();

    event_queue.roundtrip(&mut state)?;

    let manager = globals
        .bind::<wl_data_device_manager::WlDataDeviceManager, _, _>(&qh, 1..=1, ())
        .context("Failed to bind DataControlManagerV1")?;

    let seat = globals
        .bind::<wl_seat::WlSeat, _, _>(&qh, 1..=1, ())
        .context("Failed to bind WlSeat")?;

    let source = manager.create_data_source(&qh, ());
    for mime in state.data.keys() {
        source.offer(mime.clone());
    }

    let device = manager.get_data_device(&seat, &qh, ());
    device.set_selection(Some(&source), 0);
    conn.flush()?;

    while !state.quit {
        event_queue.blocking_dispatch(&mut state)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ready_data_can_be_read_more_than_once() {
        let mut data = LazyData::Ready(vec![1, 2, 3]);

        assert_eq!(data.get(), vec![1, 2, 3]);
        assert_eq!(data.get(), vec![1, 2, 3]);
    }

    #[test]
    fn lazy_data_is_cached_after_first_read() {
        let mut data = LazyData::Lazy(Box::new(|| vec![4, 5, 6]));

        assert_eq!(data.get(), vec![4, 5, 6]);
        assert_eq!(data.get(), vec![4, 5, 6]);
    }
}
