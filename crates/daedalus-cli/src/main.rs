use clap::{Parser, Subcommand};
use daedalus_domain::ImportRequest;
use daedalus_service::DaedalusService;

#[derive(Debug, Parser)]
#[command(name = "daedalus")]
#[command(about = "Operational CLI for the Daedalus model catalog")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Health,
    Config,
    List,
    Import {
        path: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        kind: Option<String>,
        #[arg(long)]
        copy: bool,
    },
    Rescan,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let service = DaedalusService::bootstrap_default("cli")?;

    match cli.command {
        Commands::Health => {
            println!("{}", serde_json::to_string_pretty(&service.health()?)?);
        }
        Commands::Config => {
            println!("{}", serde_json::to_string_pretty(&service.current_config()?)?);
        }
        Commands::List => {
            println!("{}", serde_json::to_string_pretty(&service.list_library_items()?)?);
        }
        Commands::Import { path, name, kind, copy } => {
            let item = service.import_local_file(ImportRequest {
                path,
                display_name: name,
                kind: kind.map(|value| value.parse()).transpose()?,
                copy_into_library: copy,
            })?;
            println!("{}", serde_json::to_string_pretty(&item)?);
        }
        Commands::Rescan => {
            println!("{}", serde_json::to_string_pretty(&service.rescan_library()?)?);
        }
    }

    Ok(())
}
