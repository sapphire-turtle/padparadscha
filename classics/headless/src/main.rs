//! The windowless game server (spec §8): the same API desktop runs on its loopback.

#[cfg(not(target_arch = "wasm32"))]
mod host {
    use std::{net::SocketAddr, path::Path};

    use corundum_framework::platform::dirs::AppDirs;
    use padparadscha_classics_core::COIN_TOSS;
    use padparadscha_common::Seed;
    use padparadscha_table::{Session, api};
    use serde::Deserialize;

    #[derive(Default, Deserialize)]
    struct File {
        #[serde(default)]
        serve: Serve,
    }

    #[derive(Default, Deserialize)]
    struct Serve {
        #[serde(default)]
        bind: Option<String>,
    }

    fn bind_from(dir: &Path) -> anyhow::Result<SocketAddr> {
        let text = std::fs::read_to_string(dir.join("settings.toml")).unwrap_or_default();
        let file: File = toml::from_str(&text).unwrap_or_default();
        Ok(file
            .serve
            .bind
            .as_deref()
            .unwrap_or("127.0.0.1:0")
            .parse()?)
    }

    fn bind_arg(mut args: impl Iterator<Item = String>, dir: &Path) -> anyhow::Result<SocketAddr> {
        match args.next().as_deref() {
            None => bind_from(dir),
            Some("--bind") => {
                let address = args
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("--bind needs an address"))?;
                anyhow::ensure!(args.next().is_none(), "unexpected argument");
                Ok(address.parse()?)
            }
            Some(arg) => anyhow::bail!("unexpected argument: {arg}"),
        }
    }

    #[tokio::main]
    pub async fn run() -> anyhow::Result<()> {
        tracing_subscriber::fmt()
            .json()
            .with_max_level(tracing::Level::INFO)
            .with_target(false)
            .try_init()
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        let dirs = AppDirs::new(env!("CARGO_PKG_NAME"))
            .ok_or_else(|| anyhow::anyhow!("no app directories"))?;
        let bind = bind_arg(std::env::args().skip(1), dirs.config())?;
        let session = Session::new(
            COIN_TOSS.name,
            Seed(getrandom::u64().expect("the OS's own randomness")),
        );
        let server = api::serve_sim(session, bind).await?;
        tracing::info!(addr = %server.addr(), "the game server is listening");
        server.wait_for_shutdown().await?;
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn binding_defaults_and_command_line_override() {
            let dir = std::env::temp_dir().join("padparadscha-no-such-settings");
            assert_eq!(
                bind_arg(std::iter::empty(), &dir).unwrap().to_string(),
                "127.0.0.1:0"
            );
            assert_eq!(
                bind_arg(["--bind".into(), "127.0.0.1:42".into()].into_iter(), &dir)
                    .unwrap()
                    .port(),
                42
            );
            assert!(bind_arg(["--bind".into()].into_iter(), &dir).is_err());
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> anyhow::Result<()> {
    host::run()
}
#[cfg(target_arch = "wasm32")]
fn main() {}
