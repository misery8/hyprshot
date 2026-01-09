use std::process::Command;

use anyhow::Result;
use gdk_pixbuf::{Pixbuf, PixbufLoader};
use gtk::prelude::PixbufLoaderExt;

pub fn capture_fullscreen() -> Result<Pixbuf> {
    
    let output = Command::new("grim").arg("-").output()?;

    anyhow::ensure!(output.status.success(), "grim returned non-zero status");

    let loader = PixbufLoader::new();
    loader.write(&output.stdout)?;
    loader.close()?;

    let pixbuf = loader
        .pixbuf()
        .ok_or_else(|| anyhow::anyhow!("Failed to load pixbuf"))?;

    Ok(pixbuf)
}