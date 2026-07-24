#![cfg_attr(docsrs, feature(doc_cfg))]

mod asset;
mod cargo;
mod dev;
mod fmt;
mod new;
mod ui;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "topcoat")]
pub struct TopcoatCli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Create a new pre-configured project
    New(new::NewCommand),
    /// Start a development server
    Dev(dev::DevCommand),
    /// Format topcoat `view!` macros
    Fmt(fmt::FmtCommand),
    /// Inspect assets embedded in the binary
    Asset(asset::AssetCommand),
    /// Manage premade UI components in your project
    Ui(ui::UiCommand),
}

pub async fn run() {
    let cli = TopcoatCli::parse();
    match cli.command {
        Command::New(cmd) => cmd.run(),
        Command::Ui(cmd) => cmd.run(),
        Command::Fmt(cmd) => cmd.run().await,
        Command::Dev(cmd) => cmd.run().await,
        Command::Asset(cmd) => cmd.run().await,
    }
}
