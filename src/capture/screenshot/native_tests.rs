use std::{cell::Cell, os::fd::AsRawFd};

use super::*;

fn pixel(value: u32) -> [u8; 4] {
    value.to_ne_bytes()
}

fn image(width: u32, height: u32) -> PixelImage {
    let mut data = Vec::new();
    for index in 0..width * height {
        data.extend_from_slice(&pixel(0xff00_0000 | index));
    }
    PixelImage {
        width,
        height,
        data,
    }
}

fn solid_image(width: u32, height: u32, value: u32) -> PixelImage {
    let pixel = pixel(value);
    PixelImage {
        width,
        height,
        data: pixel.repeat((width * height) as usize),
    }
}

fn captured_solid(
    x: i32,
    y: i32,
    logical_width: i32,
    logical_height: i32,
    pixel_width: u32,
    pixel_height: u32,
    value: u32,
) -> CapturedOutput {
    CapturedOutput {
        logical: LogicalRect {
            x,
            y,
            width: logical_width,
            height: logical_height,
        },
        image: solid_image(pixel_width, pixel_height, value),
    }
}

fn surface_pixel(surface: &mut ImageSurface, x: i32, y: i32) -> u32 {
    let stride = surface.stride() as usize;
    surface.flush();
    let data = surface.data().unwrap();
    let offset = y as usize * stride + x as usize * 4;
    u32::from_ne_bytes(data[offset..offset + 4].try_into().unwrap())
}

fn captured(
    x: i32,
    y: i32,
    logical_width: i32,
    logical_height: i32,
    pixel_width: u32,
    pixel_height: u32,
) -> CapturedOutput {
    CapturedOutput {
        logical: LogicalRect {
            x,
            y,
            width: logical_width,
            height: logical_height,
        },
        image: image(pixel_width, pixel_height),
    }
}

#[test]
fn backend_selection_prefers_ext_when_both_ext_globals_exist() {
    assert_eq!(select_backend(true, true, true).unwrap(), BackendKind::Ext);
}

#[test]
fn backend_selection_uses_wlr_only_when_ext_is_incomplete() {
    assert_eq!(select_backend(false, true, true).unwrap(), BackendKind::Wlr);
    assert_eq!(select_backend(true, false, true).unwrap(), BackendKind::Wlr);
    assert_eq!(
        select_backend(false, false, true).unwrap(),
        BackendKind::Wlr
    );
}

#[test]
fn backend_selection_errors_without_supported_backend() {
    assert!(select_backend(false, false, false).is_err());
}

#[test]
fn ext_runtime_failure_does_not_retry_wlr() {
    let wlr_calls = Cell::new(0);
    let result = execute_backend(
        BackendKind::Ext,
        || -> Result<()> { bail!("ext failed") },
        || {
            wlr_calls.set(wlr_calls.get() + 1);
            Ok(())
        },
    );

    assert!(result.is_err());
    assert_eq!(wlr_calls.get(), 0);
}

#[test]
fn one_output_plan_preserves_scaled_dimensions() {
    let outputs = [captured(0, 0, 100, 50, 200, 100)];
    let plan = build_composition_plan(&outputs).unwrap();

    assert_eq!((plan.width, plan.height), (200, 100));
    assert_eq!(plan.common_scale, 2.0);
    assert_eq!(
        plan.placements,
        vec![PixelRect {
            x: 0,
            y: 0,
            width: 200,
            height: 100
        }]
    );
}

#[test]
fn negative_horizontal_vertical_positions_and_gaps_are_preserved() {
    let outputs = [
        captured(-100, -50, 100, 100, 100, 100),
        captured(50, -50, 100, 100, 100, 100),
        captured(-100, 100, 100, 100, 100, 100),
    ];
    let plan = build_composition_plan(&outputs).unwrap();

    assert_eq!((plan.width, plan.height), (250, 250));
    assert_eq!(
        plan.placements[0],
        PixelRect {
            x: 0,
            y: 0,
            width: 100,
            height: 100
        }
    );
    assert_eq!(
        plan.placements[1],
        PixelRect {
            x: 150,
            y: 0,
            width: 100,
            height: 100
        }
    );
    assert_eq!(
        plan.placements[2],
        PixelRect {
            x: 0,
            y: 150,
            width: 100,
            height: 100
        }
    );
}

#[test]
fn scaled_edges_share_the_same_boundary_without_seam_or_overlap() {
    let outputs = [
        captured(0, 0, 101, 100, 101, 100),
        captured(101, 0, 99, 100, 99, 100),
    ];
    let plan = build_composition_plan(&outputs).unwrap();
    let left = plan.placements[0];
    let right = plan.placements[1];

    assert_eq!(left.x + left.width, right.x);
    assert_eq!(right.x + right.width, plan.width);
}

#[test]
fn scaled_opaque_output_keeps_edge_pixels_opaque_and_color_correct() {
    let outputs = [
        captured_solid(0, 0, 5, 5, 5, 5, 0xffff_0000),
        captured_solid(10, 0, 5, 5, 8, 8, 0xff00_00ff),
    ];
    let mut surface = compose_outputs(&outputs).unwrap();

    for y in 0..8 {
        for x in 0..8 {
            assert_eq!(surface_pixel(&mut surface, x, y), 0xffff_0000);
        }
    }
}

#[test]
fn touching_scaled_outputs_have_no_dark_or_transparent_boundary_seam() {
    let outputs = [
        captured_solid(0, 0, 5, 5, 5, 5, 0xffff_0000),
        captured_solid(5, 0, 5, 5, 8, 8, 0xff00_00ff),
    ];
    let mut surface = compose_outputs(&outputs).unwrap();

    for y in 0..8 {
        assert_eq!(surface_pixel(&mut surface, 7, y), 0xffff_0000);
        assert_eq!(surface_pixel(&mut surface, 8, y), 0xff00_00ff);
    }
}

#[test]
fn shm_backing_uses_anonymous_memfd() {
    let file = create_shm_file(4096).unwrap();
    let link = std::fs::read_link(format!("/proc/self/fd/{}", file.as_raw_fd())).unwrap();
    assert!(
        link.to_string_lossy().contains("memfd:hyprshot"),
        "unexpected SHM backing: {}",
        link.display()
    );
}

#[test]
fn composition_rejects_invalid_or_overflowing_bounds() {
    let invalid = [captured(0, 0, 0, 10, 1, 10)];
    assert!(build_composition_plan(&invalid).is_err());

    let huge = [
        CapturedOutput {
            logical: LogicalRect {
                x: i32::MIN,
                y: 0,
                width: 1,
                height: 1,
            },
            image: image(1, 1),
        },
        CapturedOutput {
            logical: LogicalRect {
                x: i32::MAX - 1,
                y: 0,
                width: 1,
                height: 1,
            },
            image: image(1, 1),
        },
    ];
    assert!(build_composition_plan(&huge).is_err());
}

#[test]
fn effective_scale_supports_exact_and_rounded_fractional_geometry() {
    assert_eq!(
        effective_scale(&captured(0, 0, 100, 100, 100, 100)).unwrap(),
        1.0
    );
    assert_eq!(
        effective_scale(&captured(0, 0, 100, 100, 200, 200)).unwrap(),
        2.0
    );
    assert_eq!(
        effective_scale(&captured(0, 0, 100, 80, 125, 100)).unwrap(),
        1.25
    );

    let rounded_125 = effective_scale(&captured(0, 0, 1093, 614, 1366, 768)).unwrap();
    assert!((1.24..1.26).contains(&rounded_125));

    let rounded_150 = effective_scale(&captured(0, 0, 1707, 1067, 2560, 1600)).unwrap();
    assert!((1.49..1.51).contains(&rounded_150));
}

#[test]
fn mixed_scale_outputs_choose_highest_effective_scale() {
    let outputs = [
        captured(0, 0, 100, 100, 125, 125),
        captured(100, 0, 100, 100, 200, 200),
    ];
    let plan = build_composition_plan(&outputs).unwrap();

    assert_eq!(plan.common_scale, 2.0);
    assert_eq!((plan.width, plan.height), (400, 200));
    assert_eq!(plan.placements[0].width, 200);
    assert_eq!(plan.placements[1].width, 200);
}

#[test]
fn unequal_effective_xy_scale_is_rejected() {
    let output = captured(0, 0, 100, 100, 125, 130);
    assert!(effective_scale(&output).is_err());
}

#[test]
fn argb_pixels_are_preserved() {
    let raw = pixel(0x7f12_3456);
    let normalized =
        normalize_shm_pixels(&raw, 1, 1, 4, ShmFormat::Argb8888, false, Transform::Normal).unwrap();

    assert_eq!(normalized.data, raw);
}

#[test]
fn xrgb_pixels_force_alpha_without_swapping_color_channels() {
    let raw = pixel(0x0012_3456);
    let normalized =
        normalize_shm_pixels(&raw, 1, 1, 4, ShmFormat::Xrgb8888, false, Transform::Normal).unwrap();

    assert_eq!(
        u32::from_ne_bytes(normalized.data.try_into().unwrap()),
        0xff12_3456
    );
}

#[test]
fn wlr_stride_padding_is_removed() {
    let mut raw = Vec::new();
    raw.extend_from_slice(&pixel(0xff00_0001));
    raw.extend_from_slice(&[9, 9, 9, 9]);
    raw.extend_from_slice(&pixel(0xff00_0002));
    raw.extend_from_slice(&[8, 8, 8, 8]);

    let normalized =
        normalize_shm_pixels(&raw, 1, 2, 8, ShmFormat::Argb8888, false, Transform::Normal).unwrap();

    assert_eq!(
        normalized.data,
        [pixel(0xff00_0001), pixel(0xff00_0002)].concat()
    );
}

#[test]
fn wlr_y_invert_flips_rows() {
    let raw = [pixel(0xff00_0001), pixel(0xff00_0002)].concat();
    let normalized =
        normalize_shm_pixels(&raw, 1, 2, 4, ShmFormat::Argb8888, true, Transform::Normal).unwrap();

    assert_eq!(
        normalized.data,
        [pixel(0xff00_0002), pixel(0xff00_0001)].concat()
    );
}

#[test]
fn all_wayland_transforms_normalize_with_expected_dimensions_and_pixels() {
    let source = PixelImage {
        width: 2,
        height: 3,
        data: [
            pixel(0xff00_0001),
            pixel(0xff00_0002),
            pixel(0xff00_0003),
            pixel(0xff00_0004),
            pixel(0xff00_0005),
            pixel(0xff00_0006),
        ]
        .concat(),
    };
    let cases = [
        (Transform::Normal, 2, 3, vec![1, 2, 3, 4, 5, 6]),
        (Transform::_90, 3, 2, vec![5, 3, 1, 6, 4, 2]),
        (Transform::_180, 2, 3, vec![6, 5, 4, 3, 2, 1]),
        (Transform::_270, 3, 2, vec![2, 4, 6, 1, 3, 5]),
        (Transform::Flipped, 2, 3, vec![2, 1, 4, 3, 6, 5]),
        (Transform::Flipped90, 3, 2, vec![6, 4, 2, 5, 3, 1]),
        (Transform::Flipped180, 2, 3, vec![5, 6, 3, 4, 1, 2]),
        (Transform::Flipped270, 3, 2, vec![1, 3, 5, 2, 4, 6]),
    ];

    for (transform, width, height, expected) in cases {
        let image = apply_transform(source.clone(), transform).unwrap();
        assert_eq!((image.width, image.height), (width, height));
        let values = image
            .data
            .as_chunks::<4>()
            .0
            .iter()
            .map(|pixel| u32::from_ne_bytes(*pixel) & 0x00ff_ffff)
            .collect::<Vec<_>>();
        assert_eq!(values, expected);
    }
}

#[test]
fn unsupported_shm_format_is_rejected() {
    let raw = [0; 4];
    assert!(
        normalize_shm_pixels(&raw, 1, 1, 4, ShmFormat::Rgb565, false, Transform::Normal,).is_err()
    );
}

#[test]
fn zero_dimensions_and_stride_height_overflow_are_rejected() {
    assert!(validate_dimensions(0, 1).is_err());
    assert!(validate_dimensions(1, 0).is_err());
    assert!(checked_buffer_size(u32::MAX, u32::MAX).is_err());
}

#[test]
fn failed_and_stopped_states_are_fatal() {
    let mut state = FrameState::new();
    state.failed = Some("failed".to_string());
    assert_eq!(terminal_frame_error(&state).as_deref(), Some("failed"));

    let mut state = FrameState::new();
    state.stopped = true;
    assert!(terminal_frame_error(&state).unwrap().contains("stopped"));
}
