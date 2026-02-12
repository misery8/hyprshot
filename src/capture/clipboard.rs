use std::env;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use anyhow::{Context, Result};

pub fn copy_to_clipboard(png_data: &[u8]) -> Result<()> {

    let current_exe = env::current_exe().context("Failed to get exe path")?;
    let mut daemon_path = current_exe.with_file_name("clipboard");
    
    if !daemon_path.exists() {
        daemon_path = PathBuf::from("/usr/lib/hyprshot/clipboard");
    }

    let mut child = Command::new(daemon_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("Failed to spawn clipboard daemon. Is the binary missing?")?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(png_data).context("Failed to pipe PNG data to daemon")?;
    }
    
    Ok(())
}