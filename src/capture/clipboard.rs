use std::io::Write;
use std::process::{Command, Stdio};

use anyhow::{Context, Result};

pub fn copy_to_clipboard(png_data: &[u8]) -> Result<()> {

    let image = image::load_from_memory_with_format(png_data, image::ImageFormat::Png)
        .context("Failed to load PNG image from memory")?;

    let mut bmp_buffer = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut bmp_buffer);

    image.write_to(&mut cursor, image::ImageFormat::Bmp)
        .context("Failed to convert image to BMP format")?;
    
    let mut child = Command::new("wl-copy")
        .args(["-t", "image/bmp"])
        .stdin(Stdio::piped())
        .spawn()
        .context("Failed to spawn wl-copy stdin")?;

    {
        let stdin = child.stdin.as_mut()
            .context("Failed to access wl-copy stdin")?;
        stdin.write_all(&bmp_buffer)
            .context("Failed to write date to clipboard pipe")?;
    }
    let status = child.wait().context("Failed to wait for wl-copy")?;
    
    anyhow::ensure!(
        status.success(),
        "wl-copy failed with exit code: {:?}",
        status.code()
    );

    Ok(())
}