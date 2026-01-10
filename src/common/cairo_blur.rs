use anyhow::{Ok, Error, bail};
use cairo::{Format, ImageSurface};

struct ARgb {
    a: u32,
    r: u32,
    g: u32,
    b: u32,
}

impl ARgb {
    fn from_u32(p: u32) -> Self {
        Self {
            a: (p >> 24) & 0xff,
            r: (p >> 16) & 0xff,
            g: (p >> 8) & 0xff,
            b: p & 0xff,
        }
    }

    fn to_u32(&self) -> u32 {
        (self.a << 24) | (self.r << 16) | (self.g << 8) | self.b
    }
}

/// Creates a blurred version of a specific area of the source surface.
/// Returns a new ImageSurface containing only the blurred area.
pub fn blur_image_surface(
    source: &ImageSurface,
    x: f64,
    y: f64,
    width: i32,
    height: i32,
    radius: i32,
) -> Result<ImageSurface, Error> {
    if radius < 1 {
        return copy_region(source, x, y, width, height);
    }

    let mut blur_surface = ImageSurface::create(Format::ARgb32, width, height)
        .map_err(|e| anyhow::anyhow!("Failed to create surface: {:?}", e))?;

    {
        let cr = cairo::Context::new(&blur_surface)
            .map_err(|e| anyhow::anyhow!("Failed to create context: {:?}", e))?;
        cr.set_operator(cairo::Operator::Source);
        cr.set_source_surface(source, -x, -y)?;
        cr.paint()?;
    }

    apply_blur_in_place(&mut blur_surface, radius)?;

    Ok(blur_surface)
}

/// Internal function to apply blur to an existing surface (In-place)
fn apply_blur_in_place(surface: &mut ImageSurface, radius: i32) -> Result<(), Error> {
    let width = surface.width() as usize;
    let height = surface.height() as usize;

    if width == 0 || height == 0 { return Ok(()); }

    let stride = surface.stride() as usize;

    let (kernel, kernel_sum) = generate_gaussian_kernel(radius)?;

    {
        let mut data = surface.data().map_err(|e| anyhow::anyhow!("Cairo error: {:?}", e))?;
        let stride_pixels = (stride / 4) as usize;

        let pixels = unsafe {
            std::slice::from_raw_parts_mut(data.as_mut_ptr() as *mut u32, stride_pixels * height)
        };

        let mut temp_vec = vec![0u32; width * height];
        let kernel_size = kernel.len();
        let half = (kernel_size / 2) as i32;

        // Horizontal pass
        for y in 0..height {
            let row_offset = y * stride_pixels;
            for x in 0..width {
                let mut acc = (0, 0, 0, 0);
                for k in 0..kernel_size {
                    let src_x = (x as i32 + (k as i32 - half)).clamp(0, width as i32 - 1) as usize;
                    let p = ARgb::from_u32(pixels[row_offset + src_x]);
                    let w = kernel[k];
                    acc.0 += p.a * w; acc.1 += p.r * w; acc.2 += p.g * w; acc.3 += p.b * w;
                }
                temp_vec[y * width + x] = ARgb {
                    a: acc.0 / kernel_sum, r: acc.1 / kernel_sum,
                    g: acc.2 / kernel_sum, b: acc.3 / kernel_sum
                }.to_u32();
            }
        }

        // Vertical pass
        for x in 0..width {
            for y in 0..height {
                let mut acc = (0, 0, 0, 0);
                for k in 0..kernel_size {
                    let src_y = (y as i32 + (k as i32 - half)).clamp(0, height as i32 - 1) as usize;
                    let p = ARgb::from_u32(temp_vec[src_y * width + x]);
                    let w = kernel[k];
                    acc.0 += p.a * w; acc.1 += p.r * w; acc.2 += p.g * w; acc.3 += p.b * w;
                }
                pixels[y * stride_pixels + x] = ARgb {
                    a: acc.0 / kernel_sum, r: acc.1 / kernel_sum,
                    g: acc.2 / kernel_sum, b: acc.3 / kernel_sum,
                }.to_u32();
            }
        }
    }

    surface.mark_dirty();
    Ok(())
}

fn copy_region(source: &ImageSurface, x: f64, y: f64, w: i32, h: i32) -> Result<ImageSurface, Error> {
    let surface = ImageSurface::create(Format::ARgb32, w, h)
        .map_err(|e| anyhow::anyhow!("Failed to create surface: {:?}", e))?;
    let cr = cairo::Context::new(&surface)?;
    cr.set_source_surface(source, -x, -y)?;
    cr.paint()?;
    Ok(surface)
}

fn generate_gaussian_kernel(radius: i32) -> Result<([u32; 17], u32), Error> {
    let clamped_radius = radius.clamp(1, 8);
    let size = (2 * clamped_radius + 1) as usize;
    let sigma = clamped_radius as f64 / 3.0;
    let mut kernel = [0u32; 17];
    let mut sum = 0u32;

    for i in 0..size {
        let x = (i as i32 - clamped_radius) as f64;
        let value = (-(x * x) / (2.0 * sigma * sigma)).exp();
        let scaled = (value * 1000.0).round() as u32;
        kernel[i] = scaled;
        sum += scaled;
    }

    #[allow(unreachable_code)]
    if sum == 0 { return bail!("Kernel sum is zero"); }
    Ok((kernel, sum))
}