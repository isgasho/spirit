use serde::Deserialize;
use spirit::prelude::*;
use spirit::{Empty, Pipeline, Spirit};
use spirit_reqwest::ReqwestClient;
use spirit_reqwest::blocking::{AtomicClient, IntoClient};

#[derive(Debug, Default, Deserialize)]
struct Cfg {
    #[serde(default)]
    client: ReqwestClient,
}

impl Cfg {
    fn client(&self) -> &ReqwestClient {
        &self.client
    }
}

const DEFAULT_CFG: &str = r#"
[client]
timeout = "5s"
enable-gzip = false
"#;

fn main() {
    env_logger::init();
    // The ::empty client would panic if used before it is configured
    let client = AtomicClient::empty();
    Spirit::<Empty, Cfg>::new()
        .config_defaults(DEFAULT_CFG)
        .with(
            Pipeline::new("http client")
                .extract_cfg(Cfg::client)
                .transform(IntoClient)
                .install(client.clone()),
        )
        .run(move |_| {
            // But by now, spirit already stored the configured client in there. Also, if we were
            // running for a longer time, it would replace it with a new one every time we change
            // the configuration.
            let page = client
                .get("https://www.rust-lang.org")
                .send()?
                .error_for_status()?
                .text()?;
            println!("{}", page);
            Ok(())
        });
}
