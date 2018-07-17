#[macro_use]
extern crate clap;
extern crate glob;
#[allow(unused_imports)]
#[macro_use]
extern crate quote;
extern crate rustfmt_nightly;
extern crate serde;
extern crate syn;
#[macro_use]
extern crate log;
extern crate env_logger;
extern crate proc_macro;
extern crate proc_macro2;

mod config;
mod project;

/// Builds CLI app metadata, especially command line arguments format
/// and parses given arguments.
fn parse_args<'a>() -> clap::ArgMatches<'a> {
    let settings = {
        use clap::AppSettings::*;
        [GlobalVersion]
    };
    clap::App::new(crate_name!())
        .version(crate_version!())
        .author(crate_authors!())
        .about(crate_description!())
        .settings(&settings)
        .arg(
            clap::Arg::with_name("src-path")
                .long("src-path")
                .takes_value(true)
                .number_of_values(1)
                .global(true)
                .help("Path to source directory (defaults to ./src)"),
        )
        .subcommand(
            clap::SubCommand::with_name("install")
                // .setting(clap::AppSettings::TrailingVarArg)
                .arg(
                    clap::Arg::with_name("mod-name")
                        .takes_value(true)
                        .multiple(true)
                        .help("Mod to be included"),
                ),
        )
        .get_matches()
}

fn main() {
    env_logger::init();

    let matches = parse_args();
    let config = config::Config::from_matches(&matches);
    let code = project::collect(&config.src_path, &config.install_mod_names);

    print!("{}", code);
}
