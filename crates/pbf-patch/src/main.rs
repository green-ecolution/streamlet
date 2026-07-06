use anyhow::Result;
use clap::{Parser, Subcommand};
use std::fs;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "pbf-patch",
    about = "Generate OSC changesets for OSM PBF patching"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Construction {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Construction { input, output } => {
            let sites = pbf_patch::verkehrsticker::fetch()?;
            println!("Fetched {} construction sites", sites.len());

            let (nodes, ways) = pbf_patch::pbf::load_car_network(&input)?;
            println!(
                "Loaded {} nodes and {} car-accessible ways",
                nodes.len(),
                ways.len()
            );

            let changed = pbf_patch::construction::changed_ways(&sites, &ways, &nodes);
            println!(
                "Writing {} changed ways to {}",
                changed.len(),
                output.display()
            );

            fs::write(&output, pbf_patch::osc::write_osc(&changed))?;
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_construction_subcommand() {
        let cli = Cli::try_parse_from([
            "pbf-patch",
            "construction",
            "--input",
            "in.pbf",
            "--output",
            "out.osc",
        ])
        .unwrap();
        let Command::Construction { input, output } = cli.command;
        assert_eq!(input, PathBuf::from("in.pbf"));
        assert_eq!(output, PathBuf::from("out.osc"));
    }
}
