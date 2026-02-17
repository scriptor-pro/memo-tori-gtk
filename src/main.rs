mod app;
mod config;
mod db;
mod paths;
mod version;

use anyhow::Result;

fn main() -> Result<()> {
    if std::env::args().any(|arg| arg == "--version") {
        println!("{}", version::VERSION);
        return Ok(());
    }

    let paths = paths::AppPaths::resolve()?;
    let config = config::AppConfig::load_or_create(&paths.config_path)?;
    let connection = db::open_and_init(&paths.db_path)?;

    app::run(config, connection)
}
