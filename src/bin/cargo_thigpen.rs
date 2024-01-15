use std::io::Write;

use clap::{Parser, Subcommand, ValueEnum};

use thigpen::{Cache, Lib};

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
}

impl ThigpenSubcommand {
    fn run(&self) {
        let path = std::env::current_dir().unwrap();

        let cargo_toml_path = {
            assert!(path.is_dir(), "Path is {}", path.to_str().unwrap());

            let mut path = path.to_path_buf();
            loop {
                path.push("Cargo.toml");
                if path.is_file() {
                    break;
                }
                if !(path.pop() && path.pop()) {
                    eprintln!(
                        "Could not find Cargo.toml in any parent of the current working directory"
                    );
                    std::process::exit(1);
                }
            }
            path
        };
        log::debug!("Found manifest at {:?}", cargo_toml_path);

        match self {
            ThigpenSubcommand::Scan => {
                let manifest = match cargo_toml::Manifest::from_path(&cargo_toml_path) {
                    Ok(manifest) => manifest,
                    Err(err) => {
                        eprintln!("Could not parse Cargo.toml - {err}");
                        std::process::exit(1);
                    }
                };
                log::debug!("Manifest: {:?}", manifest);

                if let Some(product) = manifest.lib {
                    log::debug!("Analyzing library as {}", product.name.as_ref().unwrap());
                    let mut path = path.clone();
                    path.push(product.clone().path.unwrap());
                    let crate_ = Lib::from_path(product.name.as_ref().unwrap(), path.as_path());

                    let cache = Cache {
                        name: product.name.unwrap(),
                        root: crate_,
                    };
                    cache.write().unwrap();
                } else {
                    eprintln!("thigpen does not (yet) support non-lib crates");
                    std::process::exit(1);
                }
            }
            ThigpenSubcommand::Visualize(args) => {
                let cache = Cache::read(cargo_toml_path).unwrap();

                let contents = cache.root.create_mermaid();
                if let Some(filename) = &args.output {
                    let mut outputfile = std::fs::OpenOptions::new()
                        .truncate(true)
                        .create(true)
                        .write(true)
                        .read(false)
                        .open(filename)
                        .unwrap();
                    write!(&mut outputfile, "{contents}").unwrap();
                } else {
                    let mut handle = std::io::stdout().lock();
                    write!(&mut handle, "{contents}").unwrap();
                };
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
    match args.command {
        SubCommand::Thigpen(args) => args.command.run(),
    }
}
