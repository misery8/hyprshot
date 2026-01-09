use clap::{Parser, Subcommand};

use gtk::prelude::*;
use gtk::{Application, gio};

mod action;
mod capture;
mod common;
mod modules;

use self::modules::screenshot;

#[derive(Parser)]
#[command(name = "hyprshot", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Screen,
}

const APP_ID: &str = "io.github.misery8.hyprshot";

fn main() {
    gio::resources_register_include!("compiled.gresource")
        .expect("Failed to register resources.");

    let cli = Cli::parse();

    let command = match cli.command {
        Some(cmd) => cmd,
        None => {
            println!("No command provided. Use --help for usage.");
            std::process::exit(1);
        }
    };

    let app = Application::new(Some(APP_ID), gio::ApplicationFlags::FLAGS_NONE);
        
    app.connect_activate(move |app| {

        match &command {
            Commands::Screen => {
                screenshot::run(app);
            }
        }
    });

    app.run_with_args(&[""; 0]);
}
