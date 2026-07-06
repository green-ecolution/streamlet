use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "pbf-patch", about = "Generate OSC changesets for OSM PBF patching")]
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

fn main() {
    let _cli = Cli::parse();
    todo!("wired up in the final task");
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
