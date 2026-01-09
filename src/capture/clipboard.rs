use std::io::Write;
use std::process::{Command, Stdio};

use anyhow::Result;

pub fn copy_to_clipboard(png_data: &[u8]) -> Result<()> {

    let mut child = Command::new("wl-copy")
        .args(["-t", "image/png"])
        .stdin(Stdio::piped())
        .spawn()?;

    child.stdin
        .as_mut()
        .expect("wl-copy stdin not avalailable")
        .write_all(png_data)?;

    let status = child.wait()?;
    anyhow::ensure!(status.success(), "wl-copy failed");

    Ok(())
}