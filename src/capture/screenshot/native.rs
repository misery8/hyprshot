use std::{
    collections::HashMap,
    fs::File,
    io::{Read, Seek, SeekFrom},
    os::fd::AsFd,
    sync::{Arc, Mutex, MutexGuard},
};

use anyhow::{anyhow, bail, ensure, Context as AnyhowContext, Result};
use cairo::{
    Context as CairoContext, Extend, Filter, Format as CairoFormat, ImageSurface, Operator,
};
use nix::{
    sys::memfd::{memfd_create, MFdFlags},
    unistd::ftruncate,
};
use wayland_client::{
    protocol::{
        wl_buffer::WlBuffer,
        wl_output::{self, Transform, WlOutput},
        wl_registry::{self, WlRegistry},
        wl_shm::{Format as ShmFormat, WlShm},
        wl_shm_pool::WlShmPool,
    },
    Connection, Dispatch, EventQueue, Proxy, QueueHandle, WEnum,
};
use wayland_protocols::{
    ext::{
        image_capture_source::v1::client::{
            ext_image_capture_source_v1::ExtImageCaptureSourceV1,
            ext_output_image_capture_source_manager_v1::ExtOutputImageCaptureSourceManagerV1,
        },
        image_copy_capture::v1::client::{
            ext_image_copy_capture_frame_v1::ExtImageCopyCaptureFrameV1,
            ext_image_copy_capture_manager_v1::{
                ExtImageCopyCaptureManagerV1, Options as ExtCopyOptions,
            },
            ext_image_copy_capture_session_v1::ExtImageCopyCaptureSessionV1,
        },
    },
    xdg::xdg_output::zv1::client::{
        zxdg_output_manager_v1::ZxdgOutputManagerV1, zxdg_output_v1::ZxdgOutputV1,
    },
};
use wayland_protocols_wlr::screencopy::v1::client::{
    zwlr_screencopy_frame_v1::ZwlrScreencopyFrameV1,
    zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1,
};

const BYTES_PER_PIXEL: u32 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BackendKind {
    Ext,
    Wlr,
}

fn select_backend(
    has_ext_source: bool,
    has_ext_copy: bool,
    has_wlr_v3: bool,
) -> Result<BackendKind> {
    if has_ext_source && has_ext_copy {
        Ok(BackendKind::Ext)
    } else if has_wlr_v3 {
        Ok(BackendKind::Wlr)
    } else {
        bail!("no supported Wayland screenshot backend is available")
    }
}

#[cfg(test)]
fn execute_backend<T, FExt, FWlr>(backend: BackendKind, ext: FExt, wlr: FWlr) -> Result<T>
where
    FExt: FnOnce() -> Result<T>,
    FWlr: FnOnce() -> Result<T>,
{
    match backend {
        BackendKind::Ext => ext(),
        BackendKind::Wlr => wlr(),
    }
}

#[derive(Clone)]
struct OutputRecord {
    global_name: u32,
    output: WlOutput,
    logical_x: Option<i32>,
    logical_y: Option<i32>,
    logical_width: Option<i32>,
    logical_height: Option<i32>,
    name: Option<String>,
    transform: Transform,
}

impl OutputRecord {
    fn logical_rect(&self) -> Result<LogicalRect> {
        let x = self
            .logical_x
            .ok_or_else(|| anyhow!("output {} is missing xdg-output logical x", self.label()))?;
        let y = self
            .logical_y
            .ok_or_else(|| anyhow!("output {} is missing xdg-output logical y", self.label()))?;
        let width = self.logical_width.ok_or_else(|| {
            anyhow!(
                "output {} is missing xdg-output logical width",
                self.label()
            )
        })?;
        let height = self.logical_height.ok_or_else(|| {
            anyhow!(
                "output {} is missing xdg-output logical height",
                self.label()
            )
        })?;
        ensure!(
            width > 0 && height > 0,
            "output {} has invalid logical geometry {}x{}",
            self.label(),
            width,
            height
        );

        Ok(LogicalRect {
            x,
            y,
            width,
            height,
        })
    }

    fn label(&self) -> String {
        self.name
            .clone()
            .unwrap_or_else(|| format!("output-{}", self.global_name))
    }
}

struct WaylandState {
    shm: Option<WlShm>,
    xdg_output_manager: Option<ZxdgOutputManagerV1>,
    ext_source_manager: Option<ExtOutputImageCaptureSourceManagerV1>,
    ext_copy_manager: Option<ExtImageCopyCaptureManagerV1>,
    wlr_manager: Option<ZwlrScreencopyManagerV1>,
    outputs: HashMap<u32, OutputRecord>,
    removed_output: Option<u32>,
}

impl WaylandState {
    fn new() -> Self {
        Self {
            shm: None,
            xdg_output_manager: None,
            ext_source_manager: None,
            ext_copy_manager: None,
            wlr_manager: None,
            outputs: HashMap::new(),
            removed_output: None,
        }
    }

    fn ensure_capture_alive(&self) -> Result<()> {
        if let Some(name) = self.removed_output {
            bail!("Wayland output global {name} disappeared during capture");
        }
        Ok(())
    }

    fn dispatch(&mut self, event_queue: &mut EventQueue<Self>) -> Result<()> {
        event_queue
            .blocking_dispatch(self)
            .context("failed to dispatch Wayland capture events")?;
        self.ensure_capture_alive()
    }

    fn capture_ext(
        &mut self,
        event_queue: &mut EventQueue<Self>,
        qh: &QueueHandle<Self>,
        output: &OutputRecord,
    ) -> Result<PixelImage> {
        let source_manager = self
            .ext_source_manager
            .clone()
            .ok_or_else(|| anyhow!("ext image capture source manager is unavailable"))?;
        let copy_manager = self
            .ext_copy_manager
            .clone()
            .ok_or_else(|| anyhow!("ext image copy capture manager is unavailable"))?;
        let source = source_manager.create_source(&output.output, qh, ());
        let shared = Arc::new(Mutex::new(FrameState::new()));
        let session =
            copy_manager.create_session(&source, ExtCopyOptions::empty(), qh, shared.clone());

        let constraints = loop {
            {
                let frame_state = lock_frame_state(&shared)?;
                if let Some(error) = terminal_frame_error(&frame_state) {
                    session.destroy();
                    source.destroy();
                    bail!("{error}");
                }
                if frame_state.constraints_done {
                    let width = frame_state.width;
                    let height = frame_state.height;
                    let formats = frame_state.shm_formats.clone();
                    let generation = frame_state.constraint_generation;
                    break (width, height, formats, generation);
                }
            }
            self.dispatch(event_queue)?;
        };

        let (width, height, formats, generation) = constraints;
        validate_dimensions(width, height)?;
        let format = choose_shm_format(&formats)?;
        let stride = width
            .checked_mul(BYTES_PER_PIXEL)
            .ok_or_else(|| anyhow!("ext SHM stride overflow"))?;
        let size = checked_buffer_size(stride, height)?;
        let shm = self
            .shm
            .clone()
            .ok_or_else(|| anyhow!("wl_shm is unavailable"))?;
        let mut shm_buffer = ShmBuffer::new(&shm, qh, width, height, stride, format, size)?;

        let frame = session.create_frame(qh, shared.clone());
        frame.attach_buffer(&shm_buffer.buffer);
        frame.damage_buffer(
            0,
            0,
            i32::try_from(width).context("ext frame width exceeds i32")?,
            i32::try_from(height).context("ext frame height exceeds i32")?,
        );
        frame.capture();

        let capture_result = (|| -> Result<PixelImage> {
            loop {
                {
                    let frame_state = lock_frame_state(&shared)?;
                    if let Some(error) = terminal_frame_error(&frame_state) {
                        bail!("{error}");
                    }
                    ensure!(
                        frame_state.constraints_done
                            && frame_state.constraint_generation == generation,
                        "ext image-copy constraints changed while a frame buffer was in use"
                    );
                    if frame_state.ready {
                        let raw = shm_buffer.read_bytes()?;
                        return normalize_shm_pixels(
                            &raw,
                            width,
                            height,
                            stride,
                            format,
                            false,
                            frame_state.transform,
                        );
                    }
                }
                self.dispatch(event_queue)?;
            }
        })();

        frame.destroy();
        shm_buffer.buffer.destroy();
        shm_buffer.pool.destroy();
        session.destroy();
        source.destroy();

        capture_result
    }

    fn capture_wlr(
        &mut self,
        event_queue: &mut EventQueue<Self>,
        qh: &QueueHandle<Self>,
        output: &OutputRecord,
    ) -> Result<PixelImage> {
        let manager = self
            .wlr_manager
            .clone()
            .ok_or_else(|| anyhow!("wlr-screencopy-v1 v3 is unavailable"))?;
        let shared = Arc::new(Mutex::new(FrameState::new()));
        let frame = manager.capture_output(0, &output.output, qh, shared.clone());

        let (width, height, stride, format) = loop {
            {
                let frame_state = lock_frame_state(&shared)?;
                if let Some(error) = terminal_frame_error(&frame_state) {
                    frame.destroy();
                    bail!("{error}");
                }
                if frame_state.wlr_buffer_done {
                    let format = frame_state
                        .wlr_format
                        .ok_or_else(|| anyhow!("WLR screencopy did not advertise an SHM format"))?;
                    break (
                        frame_state.width,
                        frame_state.height,
                        frame_state.wlr_stride,
                        format,
                    );
                }
            }
            self.dispatch(event_queue)?;
        };

        validate_dimensions(width, height)?;
        ensure!(
            matches!(format, ShmFormat::Argb8888 | ShmFormat::Xrgb8888),
            "unsupported WLR SHM format: {format:?}"
        );
        let minimum_stride = width
            .checked_mul(BYTES_PER_PIXEL)
            .ok_or_else(|| anyhow!("WLR minimum stride overflow"))?;
        ensure!(
            stride >= minimum_stride,
            "WLR stride {stride} is smaller than required row size {minimum_stride}"
        );
        let size = checked_buffer_size(stride, height)?;
        let shm = self
            .shm
            .clone()
            .ok_or_else(|| anyhow!("wl_shm is unavailable"))?;
        let mut shm_buffer = ShmBuffer::new(&shm, qh, width, height, stride, format, size)?;

        frame.copy(&shm_buffer.buffer);

        let capture_result = (|| -> Result<PixelImage> {
            loop {
                {
                    let frame_state = lock_frame_state(&shared)?;
                    if let Some(error) = terminal_frame_error(&frame_state) {
                        bail!("{error}");
                    }
                    if frame_state.ready {
                        let raw = shm_buffer.read_bytes()?;
                        return normalize_shm_pixels(
                            &raw,
                            width,
                            height,
                            stride,
                            format,
                            frame_state.y_invert,
                            output.transform,
                        );
                    }
                }
                self.dispatch(event_queue)?;
            }
        })();

        frame.destroy();
        shm_buffer.buffer.destroy();
        shm_buffer.pool.destroy();

        capture_result
    }
}

#[derive(Debug)]
struct FrameState {
    width: u32,
    height: u32,
    shm_formats: Vec<ShmFormat>,
    constraints_done: bool,
    constraint_generation: u64,
    ready: bool,
    failed: Option<String>,
    stopped: bool,
    transform: Transform,
    wlr_format: Option<ShmFormat>,
    wlr_stride: u32,
    wlr_buffer_done: bool,
    y_invert: bool,
}

impl FrameState {
    fn new() -> Self {
        Self {
            width: 0,
            height: 0,
            shm_formats: Vec::new(),
            constraints_done: false,
            constraint_generation: 0,
            ready: false,
            failed: None,
            stopped: false,
            transform: Transform::Normal,
            wlr_format: None,
            wlr_stride: 0,
            wlr_buffer_done: false,
            y_invert: false,
        }
    }

    fn begin_constraint_update(&mut self) {
        if self.constraints_done {
            self.width = 0;
            self.height = 0;
            self.shm_formats.clear();
            self.constraints_done = false;
        }
    }
}

fn terminal_frame_error(state: &FrameState) -> Option<String> {
    if let Some(error) = &state.failed {
        Some(error.clone())
    } else if state.stopped {
        Some("Wayland image-copy capture session stopped".to_string())
    } else {
        None
    }
}

fn lock_frame_state(shared: &Arc<Mutex<FrameState>>) -> Result<MutexGuard<'_, FrameState>> {
    shared
        .lock()
        .map_err(|_| anyhow!("Wayland frame state mutex was poisoned"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LogicalRect {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PixelImage {
    width: u32,
    height: u32,
    data: Vec<u8>,
}

#[derive(Debug, Clone)]
struct CapturedOutput {
    logical: LogicalRect,
    image: PixelImage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PixelRect {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

#[derive(Debug)]
struct CompositionPlan {
    width: i32,
    height: i32,
    #[cfg(test)]
    common_scale: f64,
    placements: Vec<PixelRect>,
}

struct ShmBuffer {
    file: File,
    pool: WlShmPool,
    buffer: WlBuffer,
    size: usize,
}

impl ShmBuffer {
    fn new(
        shm: &WlShm,
        qh: &QueueHandle<WaylandState>,
        width: u32,
        height: u32,
        stride: u32,
        format: ShmFormat,
        size: usize,
    ) -> Result<Self> {
        let file = create_shm_file(size)?;
        let pool_size = i32::try_from(size).context("SHM allocation exceeds wl_shm pool limit")?;
        let width = i32::try_from(width).context("SHM width exceeds i32")?;
        let height = i32::try_from(height).context("SHM height exceeds i32")?;
        let stride = i32::try_from(stride).context("SHM stride exceeds i32")?;
        let pool = shm.create_pool(file.as_fd(), pool_size, qh, ());
        let buffer = pool.create_buffer(0, width, height, stride, format, qh, ());

        Ok(Self {
            file,
            pool,
            buffer,
            size,
        })
    }

    fn read_bytes(&mut self) -> Result<Vec<u8>> {
        self.file
            .seek(SeekFrom::Start(0))
            .context("failed to rewind Wayland SHM file")?;
        let mut data = vec![0; self.size];
        self.file
            .read_exact(&mut data)
            .context("failed to read captured Wayland SHM buffer")?;
        Ok(data)
    }
}

fn create_shm_file(size: usize) -> Result<File> {
    ensure!(size > 0, "cannot allocate an empty Wayland SHM buffer");

    let memfd = memfd_create("hyprshot", MFdFlags::MFD_CLOEXEC)
        .context("failed to create anonymous Wayland SHM memfd")?;
    let size = i64::try_from(size).context("SHM size exceeds off_t")?;
    ftruncate(&memfd, size).context("failed to size anonymous Wayland SHM memfd")?;

    Ok(File::from(memfd))
}

fn checked_buffer_size(stride: u32, height: u32) -> Result<usize> {
    ensure!(stride > 0 && height > 0, "invalid zero-sized SHM buffer");
    let size = u64::from(stride)
        .checked_mul(u64::from(height))
        .ok_or_else(|| anyhow!("SHM stride * height overflow"))?;
    ensure!(
        size <= i32::MAX as u64,
        "SHM buffer size exceeds wl_shm pool limit"
    );
    usize::try_from(size).context("SHM buffer size exceeds usize")
}

fn validate_dimensions(width: u32, height: u32) -> Result<()> {
    ensure!(
        width > 0 && height > 0,
        "capture returned invalid dimensions {width}x{height}"
    );
    ensure!(
        width <= i32::MAX as u32 && height <= i32::MAX as u32,
        "capture dimensions exceed Cairo/Wayland integer limits"
    );
    Ok(())
}

fn choose_shm_format(formats: &[ShmFormat]) -> Result<ShmFormat> {
    if formats.contains(&ShmFormat::Argb8888) {
        Ok(ShmFormat::Argb8888)
    } else if formats.contains(&ShmFormat::Xrgb8888) {
        Ok(ShmFormat::Xrgb8888)
    } else {
        bail!("capture backend offers no supported ARGB8888/XRGB8888 SHM format")
    }
}

fn normalize_shm_pixels(
    raw: &[u8],
    width: u32,
    height: u32,
    stride: u32,
    format: ShmFormat,
    y_invert: bool,
    transform: Transform,
) -> Result<PixelImage> {
    validate_dimensions(width, height)?;
    ensure!(
        matches!(format, ShmFormat::Argb8888 | ShmFormat::Xrgb8888),
        "unsupported SHM format: {format:?}"
    );

    let row_bytes = width
        .checked_mul(BYTES_PER_PIXEL)
        .ok_or_else(|| anyhow!("pixel row size overflow"))?;
    ensure!(
        stride >= row_bytes,
        "SHM stride {stride} is smaller than pixel row {row_bytes}"
    );
    let expected = checked_buffer_size(stride, height)?;
    ensure!(
        raw.len() >= expected,
        "captured SHM buffer is shorter than advertised stride * height"
    );

    let packed_size = checked_buffer_size(row_bytes, height)?;
    let mut packed = vec![0; packed_size];
    let row_bytes_usize = usize::try_from(row_bytes).context("row size exceeds usize")?;
    let stride_usize = usize::try_from(stride).context("stride exceeds usize")?;

    for y in 0..usize::try_from(height).context("height exceeds usize")? {
        let src_start = y
            .checked_mul(stride_usize)
            .ok_or_else(|| anyhow!("source row offset overflow"))?;
        let dst_start = y
            .checked_mul(row_bytes_usize)
            .ok_or_else(|| anyhow!("destination row offset overflow"))?;
        packed[dst_start..dst_start + row_bytes_usize]
            .copy_from_slice(&raw[src_start..src_start + row_bytes_usize]);
    }

    if format == ShmFormat::Xrgb8888 {
        for pixel in packed.as_chunks_mut::<4>().0 {
            let value = u32::from_ne_bytes(*pixel);
            pixel.copy_from_slice(&(value | 0xff00_0000).to_ne_bytes());
        }
    }

    let image = PixelImage {
        width,
        height,
        data: packed,
    };
    let image = if y_invert {
        flip_vertical(image)?
    } else {
        image
    };
    apply_transform(image, transform)
}

fn flip_vertical(image: PixelImage) -> Result<PixelImage> {
    let row_bytes = usize::try_from(
        image
            .width
            .checked_mul(BYTES_PER_PIXEL)
            .ok_or_else(|| anyhow!("vertical flip row size overflow"))?,
    )
    .context("vertical flip row size exceeds usize")?;
    let mut out = vec![0; image.data.len()];
    let height = usize::try_from(image.height).context("image height exceeds usize")?;

    for y in 0..height {
        let src = y * row_bytes;
        let dst = (height - 1 - y) * row_bytes;
        out[dst..dst + row_bytes].copy_from_slice(&image.data[src..src + row_bytes]);
    }

    Ok(PixelImage { data: out, ..image })
}

fn apply_transform(image: PixelImage, transform: Transform) -> Result<PixelImage> {
    if transform == Transform::Normal {
        return Ok(image);
    }

    let width = usize::try_from(image.width).context("transform width exceeds usize")?;
    let height = usize::try_from(image.height).context("transform height exceeds usize")?;
    let (dst_width, dst_height) = match transform {
        Transform::_90 | Transform::_270 | Transform::Flipped90 | Transform::Flipped270 => {
            (height, width)
        }
        _ => (width, height),
    };
    let pixel_count = dst_width
        .checked_mul(dst_height)
        .ok_or_else(|| anyhow!("transformed pixel count overflow"))?;
    let mut out = vec![
        0;
        pixel_count
            .checked_mul(4)
            .ok_or_else(|| anyhow!("transformed byte size overflow"))?
    ];

    for y in 0..height {
        for x in 0..width {
            let (dx, dy) = match transform {
                Transform::Normal => (x, y),
                Transform::_90 => (height - 1 - y, x),
                Transform::_180 => (width - 1 - x, height - 1 - y),
                Transform::_270 => (y, width - 1 - x),
                Transform::Flipped => (width - 1 - x, y),
                Transform::Flipped90 => (height - 1 - y, width - 1 - x),
                Transform::Flipped180 => (x, height - 1 - y),
                Transform::Flipped270 => (y, x),
                _ => bail!("unknown Wayland output transform"),
            };
            let src = (y * width + x) * 4;
            let dst = (dy * dst_width + dx) * 4;
            out[dst..dst + 4].copy_from_slice(&image.data[src..src + 4]);
        }
    }

    Ok(PixelImage {
        width: u32::try_from(dst_width).context("transformed width exceeds u32")?,
        height: u32::try_from(dst_height).context("transformed height exceeds u32")?,
        data: out,
    })
}

fn scale_interval(buffer: u32, logical: i32) -> Result<(f64, f64)> {
    ensure!(
        buffer > 0 && logical > 0,
        "scale dimensions must be positive"
    );

    let logical = f64::from(logical);
    let buffer = f64::from(buffer);
    let minimum = buffer / (logical + 0.5);
    let maximum = buffer / (logical - 0.5);

    ensure!(
        minimum.is_finite() && maximum.is_finite() && minimum > 0.0 && maximum >= minimum,
        "invalid rounded scale interval"
    );
    Ok((minimum, maximum))
}

fn effective_scale(output: &CapturedOutput) -> Result<f64> {
    ensure!(
        output.logical.width > 0 && output.logical.height > 0,
        "logical output dimensions must be positive"
    );
    validate_dimensions(output.image.width, output.image.height)?;

    let scale_x = f64::from(output.image.width) / f64::from(output.logical.width);
    let scale_y = f64::from(output.image.height) / f64::from(output.logical.height);
    let (x_min, x_max) = scale_interval(output.image.width, output.logical.width)?;
    let (y_min, y_max) = scale_interval(output.image.height, output.logical.height)?;
    let overlap_min = x_min.max(y_min);
    let overlap_max = x_max.min(y_max);

    ensure!(
        overlap_min <= overlap_max,
        "output has inconsistent rounded effective X/Y scale ({scale_x} vs {scale_y})"
    );

    Ok(((scale_x + scale_y) / 2.0).clamp(overlap_min, overlap_max))
}

fn build_composition_plan(outputs: &[CapturedOutput]) -> Result<CompositionPlan> {
    ensure!(!outputs.is_empty(), "cannot compose zero captured outputs");

    let mut common_scale = 0.0_f64;
    let mut min_x = i64::MAX;
    let mut min_y = i64::MAX;
    let mut max_x = i64::MIN;
    let mut max_y = i64::MIN;

    for output in outputs {
        let scale = effective_scale(output)?;
        common_scale = common_scale.max(scale);

        let left = i64::from(output.logical.x);
        let top = i64::from(output.logical.y);
        let right = left
            .checked_add(i64::from(output.logical.width))
            .ok_or_else(|| anyhow!("logical output right edge overflow"))?;
        let bottom = top
            .checked_add(i64::from(output.logical.height))
            .ok_or_else(|| anyhow!("logical output bottom edge overflow"))?;
        ensure!(
            right > left && bottom > top,
            "invalid logical output rectangle"
        );

        min_x = min_x.min(left);
        min_y = min_y.min(top);
        max_x = max_x.max(right);
        max_y = max_y.max(bottom);
    }

    ensure!(
        common_scale.is_finite() && common_scale > 0.0,
        "invalid common output scale"
    );
    let logical_width = max_x
        .checked_sub(min_x)
        .ok_or_else(|| anyhow!("global logical width overflow"))?;
    let logical_height = max_y
        .checked_sub(min_y)
        .ok_or_else(|| anyhow!("global logical height overflow"))?;
    ensure!(
        logical_width > 0 && logical_height > 0,
        "invalid global logical bounds"
    );

    let width = scaled_edge(logical_width, common_scale)?;
    let height = scaled_edge(logical_height, common_scale)?;
    ensure!(width > 0 && height > 0, "scaled global bounds are empty");
    let width = i32::try_from(width).context("scaled global width exceeds i32")?;
    let height = i32::try_from(height).context("scaled global height exceeds i32")?;

    let mut placements = Vec::with_capacity(outputs.len());
    for output in outputs {
        let left = i64::from(output.logical.x) - min_x;
        let top = i64::from(output.logical.y) - min_y;
        let right = left + i64::from(output.logical.width);
        let bottom = top + i64::from(output.logical.height);

        let left = scaled_edge(left, common_scale)?;
        let top = scaled_edge(top, common_scale)?;
        let right = scaled_edge(right, common_scale)?;
        let bottom = scaled_edge(bottom, common_scale)?;
        ensure!(
            right > left && bottom > top,
            "scaled output destination is empty"
        );

        placements.push(PixelRect {
            x: i32::try_from(left).context("scaled output x exceeds i32")?,
            y: i32::try_from(top).context("scaled output y exceeds i32")?,
            width: i32::try_from(right - left).context("scaled output width exceeds i32")?,
            height: i32::try_from(bottom - top).context("scaled output height exceeds i32")?,
        });
    }

    Ok(CompositionPlan {
        width,
        height,
        #[cfg(test)]
        common_scale,
        placements,
    })
}

fn scaled_edge(logical_delta: i64, scale: f64) -> Result<i64> {
    ensure!(logical_delta >= 0, "scaled edge delta must be non-negative");
    let value = logical_delta as f64 * scale;
    ensure!(value.is_finite(), "scaled edge is not finite");
    ensure!(
        value <= i32::MAX as f64,
        "scaled edge exceeds Cairo integer limits"
    );
    Ok(value.round() as i64)
}

fn compose_outputs(outputs: &[CapturedOutput]) -> Result<ImageSurface> {
    let plan = build_composition_plan(outputs)?;
    let surface = ImageSurface::create(CairoFormat::ARgb32, plan.width, plan.height)
        .context("failed to allocate composed screenshot surface")?;
    let cr = CairoContext::new(&surface).context("failed to create screenshot Cairo context")?;
    cr.set_operator(Operator::Source);
    cr.set_source_rgba(0.0, 0.0, 0.0, 0.0);
    cr.paint().context("failed to clear screenshot surface")?;
    cr.set_operator(Operator::Over);

    for (output, placement) in outputs.iter().zip(&plan.placements) {
        let width = i32::try_from(output.image.width).context("source width exceeds i32")?;
        let height = i32::try_from(output.image.height).context("source height exceeds i32")?;
        let stride = width
            .checked_mul(BYTES_PER_PIXEL as i32)
            .ok_or_else(|| anyhow!("Cairo source stride overflow"))?;
        let source = ImageSurface::create_for_data(
            output.image.data.clone(),
            CairoFormat::ARgb32,
            width,
            height,
            stride,
        )
        .context("failed to create captured output surface")?;

        cr.save().context("failed to save composition context")?;
        cr.rectangle(
            f64::from(placement.x),
            f64::from(placement.y),
            f64::from(placement.width),
            f64::from(placement.height),
        );
        cr.clip();
        cr.translate(f64::from(placement.x), f64::from(placement.y));
        cr.scale(
            f64::from(placement.width) / f64::from(width),
            f64::from(placement.height) / f64::from(height),
        );
        cr.set_source_surface(&source, 0.0, 0.0)
            .context("failed to set captured output as Cairo source")?;
        let source_pattern = cr.source();
        source_pattern.set_filter(Filter::Best);
        source_pattern.set_extend(Extend::Pad);
        cr.paint().context("failed to compose captured output")?;
        cr.restore()
            .context("failed to restore composition context")?;
    }

    Ok(surface)
}

pub(super) fn capture_fullscreen() -> Result<ImageSurface> {
    let connection =
        Connection::connect_to_env().context("failed to connect to the Wayland compositor")?;
    let mut event_queue = connection.new_event_queue::<WaylandState>();
    let qh = event_queue.handle();
    let _registry = connection.display().get_registry(&qh, ());
    let mut state = WaylandState::new();

    event_queue
        .roundtrip(&mut state)
        .context("failed to discover Wayland globals")?;
    event_queue
        .roundtrip(&mut state)
        .context("failed to collect Wayland output geometry")?;
    event_queue
        .roundtrip(&mut state)
        .context("failed to complete Wayland output discovery")?;

    state.ensure_capture_alive()?;
    ensure!(
        state.shm.is_some(),
        "Wayland compositor does not expose wl_shm"
    );
    ensure!(
        state.xdg_output_manager.is_some(),
        "Wayland compositor does not expose zxdg_output_manager_v1"
    );
    ensure!(
        !state.outputs.is_empty(),
        "Wayland compositor reports no outputs"
    );

    let backend = select_backend(
        state.ext_source_manager.is_some(),
        state.ext_copy_manager.is_some(),
        state.wlr_manager.is_some(),
    )?;

    let mut outputs = state.outputs.values().cloned().collect::<Vec<_>>();
    outputs.sort_by_key(|output| output.global_name);

    let mut captured = Vec::with_capacity(outputs.len());
    for output in outputs {
        let logical = output.logical_rect()?;
        let image = match backend {
            BackendKind::Ext => state.capture_ext(&mut event_queue, &qh, &output)?,
            BackendKind::Wlr => state.capture_wlr(&mut event_queue, &qh, &output)?,
        };
        captured.push(CapturedOutput { logical, image });
    }

    state.ensure_capture_alive()?;
    compose_outputs(&captured)
}

impl Dispatch<WlRegistry, ()> for WaylandState {
    fn event(
        state: &mut Self,
        registry: &WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match event {
            wl_registry::Event::Global {
                name,
                interface,
                version,
            } => match interface.as_str() {
                "wl_shm" => {
                    state.shm = Some(registry.bind::<WlShm, _, _>(name, version.min(1), qh, ()));
                }
                "wl_output" => {
                    let output = registry.bind::<WlOutput, _, _>(name, version.min(4), qh, name);
                    state.outputs.insert(
                        name,
                        OutputRecord {
                            global_name: name,
                            output: output.clone(),
                            logical_x: None,
                            logical_y: None,
                            logical_width: None,
                            logical_height: None,
                            name: None,
                            transform: Transform::Normal,
                        },
                    );
                    if let Some(manager) = &state.xdg_output_manager {
                        manager.get_xdg_output(&output, qh, name);
                    }
                }
                "zxdg_output_manager_v1" => {
                    let manager =
                        registry.bind::<ZxdgOutputManagerV1, _, _>(name, version.min(3), qh, ());
                    for output in state.outputs.values() {
                        manager.get_xdg_output(&output.output, qh, output.global_name);
                    }
                    state.xdg_output_manager = Some(manager);
                }
                "ext_output_image_capture_source_manager_v1" => {
                    state.ext_source_manager =
                        Some(registry.bind::<ExtOutputImageCaptureSourceManagerV1, _, _>(
                            name,
                            version.min(1),
                            qh,
                            (),
                        ));
                }
                "ext_image_copy_capture_manager_v1" => {
                    state.ext_copy_manager =
                        Some(registry.bind::<ExtImageCopyCaptureManagerV1, _, _>(
                            name,
                            version.min(1),
                            qh,
                            (),
                        ));
                }
                "zwlr_screencopy_manager_v1" if version >= 3 => {
                    state.wlr_manager =
                        Some(registry.bind::<ZwlrScreencopyManagerV1, _, _>(name, 3, qh, ()));
                }
                _ => {}
            },
            wl_registry::Event::GlobalRemove { name } if state.outputs.contains_key(&name) => {
                state.removed_output = Some(name);
            }
            _ => {}
        }
    }
}

impl Dispatch<WlOutput, u32> for WaylandState {
    fn event(
        state: &mut Self,
        _: &WlOutput,
        event: wl_output::Event,
        global_name: &u32,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let Some(output) = state.outputs.get_mut(global_name) else {
            return;
        };

        match event {
            wl_output::Event::Geometry {
                transform: WEnum::Value(transform),
                ..
            } => output.transform = transform,
            wl_output::Event::Name { name } => output.name = Some(name),
            _ => {}
        }
    }
}

impl Dispatch<ZxdgOutputManagerV1, ()> for WaylandState {
    fn event(
        _: &mut Self,
        _: &ZxdgOutputManagerV1,
        _: <ZxdgOutputManagerV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZxdgOutputV1, u32> for WaylandState {
    fn event(
        state: &mut Self,
        _: &ZxdgOutputV1,
        event: <ZxdgOutputV1 as Proxy>::Event,
        global_name: &u32,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let Some(output) = state.outputs.get_mut(global_name) else {
            return;
        };

        use wayland_protocols::xdg::xdg_output::zv1::client::zxdg_output_v1::Event;
        match event {
            Event::LogicalPosition { x, y } => {
                output.logical_x = Some(x);
                output.logical_y = Some(y);
            }
            Event::LogicalSize { width, height } => {
                output.logical_width = Some(width);
                output.logical_height = Some(height);
            }
            Event::Name { name } => output.name = Some(name),
            _ => {}
        }
    }
}

impl Dispatch<WlShm, ()> for WaylandState {
    fn event(
        _: &mut Self,
        _: &WlShm,
        _: <WlShm as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlShmPool, ()> for WaylandState {
    fn event(
        _: &mut Self,
        _: &WlShmPool,
        _: <WlShmPool as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlBuffer, ()> for WaylandState {
    fn event(
        _: &mut Self,
        _: &WlBuffer,
        _: <WlBuffer as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ExtOutputImageCaptureSourceManagerV1, ()> for WaylandState {
    fn event(
        _: &mut Self,
        _: &ExtOutputImageCaptureSourceManagerV1,
        _: <ExtOutputImageCaptureSourceManagerV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ExtImageCaptureSourceV1, ()> for WaylandState {
    fn event(
        _: &mut Self,
        _: &ExtImageCaptureSourceV1,
        _: <ExtImageCaptureSourceV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ExtImageCopyCaptureManagerV1, ()> for WaylandState {
    fn event(
        _: &mut Self,
        _: &ExtImageCopyCaptureManagerV1,
        _: <ExtImageCopyCaptureManagerV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ExtImageCopyCaptureSessionV1, Arc<Mutex<FrameState>>> for WaylandState {
    fn event(
        _: &mut Self,
        _: &ExtImageCopyCaptureSessionV1,
        event: <ExtImageCopyCaptureSessionV1 as Proxy>::Event,
        shared: &Arc<Mutex<FrameState>>,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let Ok(mut state) = shared.lock() else {
            return;
        };

        use wayland_protocols::ext::image_copy_capture::v1::client::ext_image_copy_capture_session_v1::Event;
        match event {
            Event::BufferSize { width, height } => {
                state.begin_constraint_update();
                state.width = width;
                state.height = height;
            }
            Event::ShmFormat {
                format: WEnum::Value(format),
            } => {
                state.begin_constraint_update();
                if !state.shm_formats.contains(&format) {
                    state.shm_formats.push(format);
                }
            }
            Event::ShmFormat { .. } => {
                state.begin_constraint_update();
            }
            Event::Done => {
                state.constraints_done = true;
                state.constraint_generation = state.constraint_generation.saturating_add(1);
            }
            Event::Stopped => state.stopped = true,
            _ => {}
        }
    }
}

impl Dispatch<ExtImageCopyCaptureFrameV1, Arc<Mutex<FrameState>>> for WaylandState {
    fn event(
        _: &mut Self,
        _: &ExtImageCopyCaptureFrameV1,
        event: <ExtImageCopyCaptureFrameV1 as Proxy>::Event,
        shared: &Arc<Mutex<FrameState>>,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let Ok(mut state) = shared.lock() else {
            return;
        };

        use wayland_protocols::ext::image_copy_capture::v1::client::ext_image_copy_capture_frame_v1::Event;
        match event {
            Event::Transform {
                transform: WEnum::Value(transform),
            } => state.transform = transform,
            Event::Ready => state.ready = true,
            Event::Failed { reason } => {
                state.failed = Some(format!("ext image-copy capture frame failed: {reason:?}"));
            }
            _ => {}
        }
    }
}

impl Dispatch<ZwlrScreencopyManagerV1, ()> for WaylandState {
    fn event(
        _: &mut Self,
        _: &ZwlrScreencopyManagerV1,
        _: <ZwlrScreencopyManagerV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwlrScreencopyFrameV1, Arc<Mutex<FrameState>>> for WaylandState {
    fn event(
        _: &mut Self,
        _: &ZwlrScreencopyFrameV1,
        event: <ZwlrScreencopyFrameV1 as Proxy>::Event,
        shared: &Arc<Mutex<FrameState>>,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let Ok(mut state) = shared.lock() else {
            return;
        };

        use wayland_protocols_wlr::screencopy::v1::client::zwlr_screencopy_frame_v1::Event;
        match event {
            Event::Buffer {
                format: WEnum::Value(format),
                width,
                height,
                stride,
            } => {
                state.wlr_format = Some(format);
                state.width = width;
                state.height = height;
                state.wlr_stride = stride;
            }
            Event::Buffer {
                width,
                height,
                stride,
                ..
            } => {
                state.width = width;
                state.height = height;
                state.wlr_stride = stride;
            }
            Event::BufferDone => state.wlr_buffer_done = true,
            Event::Flags {
                flags: WEnum::Value(flags),
            } => state.y_invert = flags.bits() & 1 != 0,
            Event::Ready { .. } => state.ready = true,
            Event::Failed => {
                state.failed = Some("WLR screencopy frame failed".to_string());
            }
            Event::LinuxDmabuf { .. } => {}
            _ => {}
        }
    }
}

#[cfg(test)]
#[path = "native_tests.rs"]
mod tests;
