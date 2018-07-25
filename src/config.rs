//! Defines data structures of command line arguments.

use clap;

#[derive(Debug)]
pub struct Config {
    pub src_path: Option<String>,
    pub main_path: Option<String>,
    pub install_mod_names: Vec<String>,
}

impl Config {
    pub fn from_matches(gm: &clap::ArgMatches) -> Self {
        let install_mod_names = match gm.subcommand() {
            ("install", None) => {
                warn!("Nothing to install");
                Vec::new()
            }
            ("install", Some(sm)) => {
                let mod_names = sm
                    .values_of("mod-name")
                    .into_iter()
                    .flat_map(|names| names)
                    .map(|name| name.to_owned())
                    .collect::<Vec<_>>();
                trace!("install {:?}", mod_names);
                mod_names
            }
            _ => {
                error!("unknown subcommand");
                vec![]
            }
        };

        let src_path = gm.value_of("src-path").map(|s| s.to_owned());

        let main_path = gm.value_of("main-path").map(|s| s.to_owned());

        Config {
            src_path,
            main_path,
            install_mod_names,
        }
    }
}
