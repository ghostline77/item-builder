mod catalogue;
mod export;
mod icons;
mod project;
mod recolour;
mod server;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Open the local item mod builder.
    Serve {
        #[arg(long, default_value = "../items/dist/catalogue")]
        catalogue: PathBuf,
        #[arg(long, default_value = "work/projects")]
        projects: PathBuf,
        #[arg(long, default_value_t = 0)]
        port: u16,
        #[arg(long)]
        game: Option<PathBuf>,
        #[arg(long, requires = "game")]
        schema: Option<PathBuf>,
    },
    /// Validate and package an authored project without the browser.
    Export {
        #[arg(long, default_value = "../items/dist/catalogue")]
        catalogue: PathBuf,
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        game: Option<PathBuf>,
        #[arg(long, requires = "game")]
        schema: Option<PathBuf>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Serve {
            catalogue,
            projects,
            port,
            game,
            schema,
        } => server::serve(catalogue, projects, port, game.map(|game| (game, schema))).await,
        Command::Export {
            catalogue,
            project,
            out,
            game,
            schema,
        } => {
            let catalogue = catalogue::Catalogue::load(&catalogue)?;
            let project: project::Project = serde_json::from_slice(&std::fs::read(project)?)?;
            let native = game
                .map(|game| recolour::Native::open(game, schema))
                .transpose()?;
            let package = export::package(&catalogue, &project, native.as_ref())?;
            anyhow::ensure!(!out.exists(), "output already exists");
            if let Some(parent) = out.parent().filter(|p| !p.as_os_str().is_empty()) {
                std::fs::create_dir_all(parent)?;
            }
            use std::io::Write;
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&out)?;
            file.write_all(&package)?;
            println!("{}", out.display());
            Ok(())
        }
    }
}
