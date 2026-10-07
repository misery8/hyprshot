use anyhow::Result;
use cairo::ImageSurface;

pub fn capture_fullscreen() -> Result<ImageSurface> {
    super::native::capture_fullscreen()
}
