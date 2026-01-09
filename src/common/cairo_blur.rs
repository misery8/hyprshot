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
        let r = self.r.min(self.a);
        let g = self.g.min(self.a);
        let b = self.b.min(self.a);
        
        (self.a << 24) | (r << 16) | (g << 8) | b
    }
}

pub fn blur_image_surface_v2(surface: &ImageSurface, radius: i32) -> Result<(), Error> {
    if radius < 1 { return Ok(()) }

    if surface.format() != Format::ARgb32 {
        bail!("Blur only supports ARGB surface (got {:?})", surface.format())
    }

    let width = surface.width() as usize;
    let height = surface.height() as usize;

    if width == 0 || height == 0 {
        return Ok(());
    }

    let stride = surface.stride() as usize;
    if stride % 4 != 0 {
        bail!("Unexpected stride not devisible by 4");
    }
    let stride_pixels = stride / 4;

    let (kernel, kernel_sum) = generate_gaussian_kernel(radius)?;

    surface.with_data(|data| {
        
        let pixels = unsafe {
            std::slice::from_raw_parts_mut(data.as_ptr() as *mut u32, stride_pixels * height)
        };

        let mut temp_vec = vec![0u32; width * height];
        let kernel_size = kernel.len();
        let half = (kernel_size / 2) as i32;

        // Horizontal
        for y in 0..height {
            for x in 0..width {

                let mut acc = (0, 0, 0, 0);

                for k in 0..kernel_size {
                    let offset = k as i32 - half;
                    let src_x = (x as i32 + offset).clamp(0, width as i32 - 1) as usize;
                    let src_pixel = pixels[y * stride_pixels + src_x];
                    let color = ARgb::from_u32(src_pixel);
                    let weight = kernel[k];
                    
                    acc.0 += color.a * weight; acc.1 += color.r * weight;
                    acc.2 += color.g * weight; acc.3 += color.b * weight;
                }
                
                temp_vec[y * width + x] = ARgb {
                    a: acc.0 / kernel_sum, r: acc.1 / kernel_sum,
                    g: acc.2 / kernel_sum, b: acc.3 / kernel_sum
                }.to_u32();
            }
        }

        // Vertical
        for x in 0..width {
            for y in 0..height {
                
                let mut acc = (0, 0, 0, 0);

                for k in 0..kernel_size {
                    let offset = k as i32 - half;
                    let src_y = (y as i32 + offset).clamp(0, height as i32 - 1) as usize;
                    let src_pixel = temp_vec[src_y * width + x];
                    let color = ARgb::from_u32(src_pixel);
                    let weight = kernel[k];

                    acc.0 += color.a * weight; acc.1 += color.r * weight;
                    acc.2 += color.g * weight; acc.3 += color.b * weight;
                }

                pixels[y * stride_pixels + x] = ARgb {
                    a: acc.0 / kernel_sum, r: acc.1 / kernel_sum,
                    g: acc.2 / kernel_sum, b: acc.3 / kernel_sum,
                }.to_u32();
            }
        }

    })?;

    surface.mark_dirty();

    Ok(())

}

fn generate_gaussian_kernel(radius: i32) -> Result<([u32; 17], u32), Error> {
    let clamped_radius = radius.clamp(1, 8);
    let size = (2 * clamped_radius + 1) as usize;
    if size > 17 {
        bail!("Radius too large (max supported: 8)");
    }

    let sigma = clamped_radius as f64 / 3.0;
    let mut kernel = [0u32; 17];
    let mut sum = 0u32;

    for i in 0..size {
        let x = (i as i32  - clamped_radius) as f64;
        let value = (-x * x / (2.0 * sigma * sigma)).exp();
        
        let scaled = (value * 1000.0).round() as u32;
        kernel[i] = scaled;
        sum += scaled;
    }

    for i in size..17 {
        kernel[i] = 0;
    }

    if sum == 0 {
        for i in 0..size {
            kernel[i] = 1;
        }
        sum = size as u32;
    }

    Ok((kernel, sum))
}
