//! `wl_shm` buffers filled from premultiplied RGBA pixels.

use std::fs::File;
use std::io;
use std::os::fd::AsFd;
use std::os::unix::fs::FileExt;

use rustix::fs::{MemfdFlags, memfd_create};
use wayland_client::protocol::wl_buffer::WlBuffer;
use wayland_client::protocol::wl_shm::{Format, WlShm};
use wayland_client::{Dispatch, QueueHandle};

const BYTES_PER_PIXEL: usize = 4;
/// Pixels converted per write, so no second full-size copy exists.
const CHUNK_PIXELS: usize = 4096;

/// A `width`×`height` ARGB8888 buffer holding `premultiplied_rgba`.
pub fn buffer<State>(
    shm: &WlShm,
    queue: &QueueHandle<State>,
    width: u32,
    height: u32,
    premultiplied_rgba: &[u8],
) -> io::Result<WlBuffer>
where
    State: Dispatch<WlBuffer, ()>
        + Dispatch<wayland_client::protocol::wl_shm_pool::WlShmPool, ()>
        + 'static,
{
    let invalid = || io::Error::new(io::ErrorKind::InvalidInput, "buffer size out of range");
    let length = premultiplied_rgba.len();
    let stride = width as usize * BYTES_PER_PIXEL;
    if length != stride * height as usize {
        return Err(invalid());
    }
    let file = File::from(memfd_create("crown-androidd-splash", MemfdFlags::CLOEXEC)?);
    file.set_len(length as u64)?;
    write_bgra(&file, premultiplied_rgba)?;
    let pool = shm.create_pool(
        file.as_fd(),
        i32::try_from(length).map_err(|_| invalid())?,
        queue,
        (),
    );
    let side = |value: u32| i32::try_from(value).map_err(|_| invalid());
    let buffer = pool.create_buffer(
        0,
        side(width)?,
        side(height)?,
        i32::try_from(stride).map_err(|_| invalid())?,
        Format::Argb8888,
        queue,
        (),
    );
    pool.destroy();
    Ok(buffer)
}

/// ARGB8888 is BGRA in memory on little-endian machines.
fn write_bgra(file: &File, rgba: &[u8]) -> io::Result<()> {
    let mut chunk = Vec::with_capacity(CHUNK_PIXELS * BYTES_PER_PIXEL);
    let mut offset = 0;
    for pixels in rgba.chunks(CHUNK_PIXELS * BYTES_PER_PIXEL) {
        chunk.clear();
        chunk.extend(
            pixels
                .chunks_exact(BYTES_PER_PIXEL)
                .flat_map(|pixel| [pixel[2], pixel[1], pixel[0], pixel[3]]),
        );
        file.write_all_at(&chunk, offset)?;
        offset += chunk.len() as u64;
    }
    Ok(())
}
