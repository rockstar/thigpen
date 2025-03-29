use clap::{Parser, Subcommand, ValueEnum};

use thigpen::Project;

#[derive(Clone, Debug, Default, ValueEnum)]
enum OutputType {
    #[default]
    Mermaid,
}

impl std::fmt::Display for OutputType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Mermaid => write!(f, "mermaid"),
        }
    }
}

#[derive(Debug, Parser)]
struct VisualizeArgs {
    #[arg(long, default_value_t = OutputType::Mermaid)]
    output_type: OutputType,
    #[arg(short)]
    output: Option<String>,
}

#[derive(Debug, Subcommand)]
enum ThigpenSubcommand {
    Scan,
    #[clap(visible_alias("vis"))]
    Visualize(VisualizeArgs),
    Clean,
}

impl ThigpenSubcommand {
    fn run(&self) {
        let path = std::env::current_dir().unwrap();

        match self {
            ThigpenSubcommand::Scan => {
                let project = match Project::from_path(&path) {
                    Ok(project) => project,
                    Err(err) => {
                        eprintln!("{}", err);
                        std::process::exit(1);
                    }
                };
                if let Err(err) = project.cache() {
                    eprintln!("Unable to write project scan cache. {}", err);
                    std::process::exit(1);
                }
            }
            ThigpenSubcommand::Visualize(_) => {
                let _project = match Project::from_cache(&path) {
                    Ok(project) => project,
                    Err(err) => {
                        eprintln!("Unable to read project scan cache. {}", err);
                        std::process::exit(1);
                    }
                };
                unimplemented!("visualizations are not implemented (yet)");
            }
            ThigpenSubcommand::Clean => {
                let path = path.to_path_buf();
                let cache_path = {
                    assert!(path.is_dir(), "Path is {}", path.to_str().unwrap());
        
                    let mut path = path.to_path_buf();
                    loop {
                        path.push(".thigpen.cache");
                        if path.is_file() {
                            break;
                        }
                        if !(path.pop() && path.pop()) {
                            eprintln!(
                                "Could not find thigpen cache file in any parent of the current working directory"
                            );
                            std::process::exit(1);
                        }
                    }
                    path
                };
                std::fs::remove_file(cache_path).unwrap();
            }
        }
    }
}

#[derive(Parser, Debug)]
struct ThigpenArgs {
    #[arg(long, default_value_t = false)]
    debug: bool,
    #[clap(subcommand)]
    command: ThigpenSubcommand,
}

// As this is a cargo plugin, this subcommand
// allows the pass-through from cargo to the
// next level. Without this, clap is confused about
// `cargo thigpen`.
#[derive(Debug, Subcommand)]
enum SubCommand {
    Thigpen(ThigpenArgs),
}

#[derive(Parser, Debug)]
#[command(version, bin_name = "cargo")]
struct Args {
    #[clap(subcommand)]
    command: SubCommand,
}

fn main() {
    let args = Args::parse();
    let SubCommand::Thigpen(args) = args.command;
    args.command.run();
}
