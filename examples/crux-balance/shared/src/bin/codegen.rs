//! Generates the Swift/Kotlin types for `Event`, `ViewModel` and `Effect`.
//!
//! ```bash
//! cargo run -p shared --bin codegen --features codegen -- --language swift --output-dir generated
//! ```

use {
    anyhow::Result,
    clap::{Parser, ValueEnum},
    crux_core::type_generation::facet::{Config, TypeRegistry},
    log::info,
    shared::Balance,
    std::path::PathBuf,
};

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum Language {
    Swift,
    Kotlin,
}

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long, value_enum)]
    language: Language,
    #[arg(short, long)]
    output_dir: PathBuf,
}

fn main() -> Result<()> {
    pretty_env_logger::init();
    let args = Args::parse();

    let typegen_app = TypeRegistry::new().register_app::<Balance>()?.build()?;

    let name = match args.language {
        Language::Swift => "App",
        Language::Kotlin => "dev.spume.balance",
    };
    let config = Config::builder(name, &args.output_dir).build();

    match args.language {
        Language::Swift => {
            info!("Typegen for Swift");
            typegen_app.swift(&config)?;
        }
        Language::Kotlin => {
            info!("Typegen for Kotlin");
            typegen_app.kotlin(&config)?;
        }
    }

    Ok(())
}
