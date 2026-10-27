//! revenant
//!
//! recover deleted, hidden and purged photos from an encrypted iOS backup.
//! the backup is read only; all output lands inside a workspace directory

mod backup;
mod commands;
mod crypto;
mod keybag;

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

use commands::Ctx;

#[derive(Parser)]
#[command(
    name = "revenant",
    version,
    about = "recover deleted, hidden and purged photos from an encrypted iOS backup"
)]
struct Cli {
    /// path to the backup directory (the folder holding Manifest.plist)
    #[arg(long)]
    backup: PathBuf,

    /// workspace for decrypted databases and recovered output
    #[arg(long, default_value = "workspace")]
    work: PathBuf,

    /// report totals only; do not write any csv or recovered media
    #[arg(short = 'c', long)]
    count_only: bool,

    /// thumbnails: export orphans even if the match rate looks unreliable
    #[arg(long)]
    force: bool,

    /// media: also decrypt the CPL cloud-staged assets
    #[arg(long = "include-all")]
    include_all: bool,

    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// decrypt Manifest.db and Photos.sqlite into the workspace
    Decrypt,
    /// carve deleted ZASSET rows out of Photos.sqlite free pages
    Carve,
    /// recover thumbnails for hidden, recently-deleted and purged assets
    Thumbnails,
    /// decrypt whatever original media is physically present
    Media,
    /// run decrypt, carve, thumbnails and media in order
    All,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let ctx = Ctx {
        backup: cli.backup,
        work: cli.work,
        count_only: cli.count_only,
        force: cli.force,
        include_all: cli.include_all,
    };

    match cli.cmd {
        Cmd::Decrypt => commands::decrypt(&ctx)?,
        Cmd::Carve => commands::carve(&ctx)?,
        Cmd::Thumbnails => commands::thumbnails(&ctx)?,
        Cmd::Media => commands::media(&ctx)?,
        Cmd::All => {
            commands::decrypt(&ctx)?;
            commands::carve(&ctx)?;
            commands::thumbnails(&ctx)?;
            commands::media(&ctx)?;
        }
    }
    Ok(())
}
