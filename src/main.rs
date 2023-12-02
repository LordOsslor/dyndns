use clap::Parser;
use config::Config;
use simple_logger::SimpleLogger;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::fs::File;
use tokio::io::AsyncReadExt;
mod api;
mod config;
mod misc_serialization;
mod records;

#[derive(Parser, Debug)]
struct CliArgs {
    #[arg(short,long,default_value=clap::builder::OsStr::from("config.toml"))]
    config: PathBuf,
}

async fn read_config(config_path: PathBuf) -> Result<Config, String> {
    log::info!(
        "Opening config file at {}",
        config_path.to_str().unwrap_or("(Non utf-8 string)")
    );
    let mut config_file = File::open(config_path)
        .await
        .or_else(|e| Err(format!("Could not open config file: {e}")))?;

    let mut config_string = String::new();
    config_file
        .read_to_string(&mut config_string)
        .await
        .or_else(|e| Err(format!("Could not read config file: {e}")))?;

    toml::from_str(&config_string)
        .or_else(|e| Err(format!("Could not parse config file: {e}").into()))
}

#[tokio::main]
async fn main() {
    SimpleLogger::new()
        .with_level(log::LevelFilter::Info)
        .env()
        .init()
        .expect("Logger should be initializable in main function");

    log::debug!("Parsing CLI args");
    let cli = CliArgs::parse();
    log::debug!("CLI Args: {:?}", cli);

    let conf = match read_config(cli.config).await {
        Ok(conf) => conf,
        Err(e) => {
            log::error!("Error while reading config: {e}");
            return;
        }
    };

    let mut total_search_fields = 0;
    for zone in &conf.zones {
        total_search_fields += zone.search.len();
    }

    log::info!(
        "Found configurations for {} zones with {} total search rules",
        &conf.zones.len(),
        total_search_fields
    );

    let client = Arc::new(reqwest::Client::new());
    log::info!("Getting ip addresses");
    let addr =
        match api::get_ip_addresses(conf.ipv4_service, conf.ipv6_service, client.clone()).await {
            Ok(v) => v,
            Err(e) => {
                log::error!("Could not get ip addresses: {}", e);
                return;
            }
        };

    log::info!("Got {}", api::address_tuple_to_string(addr));

    for zone in conf.zones {
        let id = &zone.identifier.clone();
        match api::patch_zone(zone, client.clone(), addr).await {
            Ok(i) => log::info!("(\"{id}\"): Patched {i} records"),
            Err(e) => log::error!("\"{id}\": Fatal error while patching records: {e}"),
        };
    }
}
