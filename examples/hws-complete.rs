//! A hello world service
//!
//! This version of a hello world service demonstrates a wide range of the possibilities and tools
//! spirit offers.
//!
//! It listens on one or more ports and greets with hello world (or other configured message) over
//! HTTP. It includes logging and daemonization.
//!
//! It allows reconfiguring everything at runtime ‒ change the config file(s), send SIGHUP to it
//! and it'll reload it.

use std::sync::Arc;

use hyper::server::Builder;
use hyper::service::service_fn_ok;
use hyper::{Body, Request, Response};
use log::{debug, trace};
use serde::{Deserialize, Serialize};
use spirit::prelude::*;
use spirit::{Pipeline, Spirit};
use spirit_cfg_helpers::Opts as CfgOpts;
use spirit_daemonize::{Daemon, Opts as DaemonOpts};
use spirit_hyper::{BuildServer, HyperServer};
use spirit_log::{Cfg as Logging, CfgAndOpts as LogBoth, Opts as LogOpts};
use spirit_tokio::either::Either;
use spirit_tokio::net::limits::WithLimits;
#[cfg(unix)]
use spirit_tokio::net::unix::UnixListen;
use spirit_tokio::runtime::ThreadPoolConfig;
use spirit_tokio::TcpListen;
use structdoc::StructDoc;
use structopt::StructOpt;

// The command line arguments we would like our application to have.
//
// Here we build it from prefabricated fragments provided by the `spirit-*` crates. Of course we
// could also roll our own.
//
// The spirit will add some more options on top of that ‒ it'll be able to accept
// `--config-override` to override one or more config option on the command line and it'll accept
// an optional list of config files and config directories.
//
// Note that this doc comment gets printed as part of the `--help` message, you can include
// authors, etc:
/// A Hello World Service.
///
/// Will listen on some HTTP sockets and greet every client that comes with a configured message,
/// by default „hello world“.
///
/// You can play with the options, configuration, runtime-reloading (by SIGHUP), etc.
#[structopt(
    version = "1.0.0-example", // Taken from Cargo.toml if not specified
    author,
)]
#[derive(Clone, Debug, StructOpt)]
struct Opts {
    // Adds the `--daemonize` and `--foreground` options.
    #[structopt(flatten)]
    daemon: DaemonOpts,

    // Adds the `--log` and `--log-module` options.
    #[structopt(flatten)]
    logging: LogOpts,

    // Adds the --help-config and --dump-config options
    #[structopt(flatten)]
    cfg_opts: CfgOpts,
}

impl Opts {
    fn logging(&self) -> LogOpts {
        self.logging.clone()
    }
    fn cfg_opts(&self) -> &CfgOpts {
        &self.cfg_opts
    }
}

/// An application specific configuration.
///
/// For the Hello World Service, we configure just the message to send.
#[derive(Clone, Debug, Default, Deserialize, StructDoc, Serialize)]
struct Ui {
    /// The message to send.
    msg: String,
}

/// Similarly, each transport we listen on will carry its own signature.
///
/// Well, optional signature. It may be missing.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, StructDoc, Serialize)]
struct Signature {
    /// A signature appended to the message.
    ///
    /// May be different on each listening port.
    #[serde(skip_serializing_if = "Option::is_none")]
    signature: Option<String>,
}

/// Configuration of a http server.
///
/// The `HttpServer` could be enough. It would allow configuring the listening port and a whole
/// bunch of other details about listening (how many accepting tasks there should be in parallel,
/// on what interface to listen, TCP keepalive, HTTP keepalive...).
///
/// But we actually want something even more crazy. We want our users to be able to use on both
/// normal http over TCP on some port as well as on unix domain sockets. Don't say you've never
/// heard of HTTP over unix domain sockets...
///
/// So when the user puts `port = 1234`, it listens on TCP. If there's `path =
/// "/tmp/path/to/socket"`, it listens on http.
///
/// We also bundle the optional signature inside of that thing.
#[cfg(unix)]
type ListenSocket = WithLimits<Either<TcpListen<Signature>, UnixListen<Signature>>>;

#[cfg(not(unix))]
type ListenSocket = WithLimits<TcpListen<Signature>>;

type Server = HyperServer<ListenSocket>;

#[cfg(unix)]
fn extract_signature(listen: &Server) -> &Option<String> {
    match &listen.transport.listener {
        Either::A(tcp) => &tcp.extra_cfg.signature,
        Either::B(unix) => &unix.extra_cfg.signature,
    }
}

#[cfg(not(unix))]
fn extract_signature(listen: &Server) -> &Option<String> {
    &listen.transport.listener.extra_cfg.signature
}

/// Putting the whole configuration together.
///
/// Note that here too, the doc comments can become part of the user help ‒ the `--help-config`
/// this time.
#[derive(Clone, Debug, Default, Deserialize, StructDoc, Serialize)]
struct Cfg {
    /// Deamonization stuff
    ///
    /// Like the user to switch to, working directory or if it should actually daemonize.
    #[serde(default)]
    daemon: Daemon,

    /// The logging.
    ///
    /// This allows multiple logging destinations in parallel, configuring the format, timestamp
    /// format, destination.
    #[serde(default, skip_serializing_if = "Logging::is_empty")]
    logging: Logging,

    /// Where to listen on.
    ///
    /// This allows multiple listening ports at once, both over ordinary TCP and on unix domain
    /// stream sockets.
    listen: Vec<Server>,

    /// The user interface.
    ui: Ui,

    /// The work threadpool.
    ///
    /// This is for performance tuning.
    threadpool: ThreadPoolConfig,
}

impl Cfg {
    fn logging(&self) -> Logging {
        self.logging.clone()
    }
    fn listen(&self) -> &Vec<Server> {
        &self.listen
    }
    fn threadpool(&self) -> ThreadPoolConfig {
        self.threadpool.clone()
    }
}

/// Let's bake some configuration in.
///
/// We wouldn't have to do that, but bundling a piece of configuration inside makes sure we can
/// start without one.
const DEFAULT_CONFIG: &str = r#"
[daemon]
pid-file = "/tmp/hws"
workdir = "/"

[[logging]]
level = "DEBUG"
type = "stderr"
clock = "UTC"
per-module = { hws_complete = "TRACE", hyper = "INFO", tokio = "INFO" }
format = "extended"

[[listen]]
port = 5678
host = "127.0.0.1"
http-mode = "http1-only"
backlog = 256
scale = 2
signature = "IPv4"
reuse-addr = true

[[listen]]
port = 5678
host = "::1"
http-mode = "http1-only"
backlog = 256
scale = 2
only-v6 = true
signature = "IPv6"
max-conn = 20
reuse-addr = true

[[listen]]
# This one will be rejected on Windows, because it'll turn off the unix domain socket support.
path = "/tmp/hws.socket"
http-mode = "http1-only"
backlog = 256
scale = 2
error-sleep = "100ms"

[ui]
msg = "Hello world"

[threadpool]
async-threads = 2
keep-alive = "15s"
"#;

/// This is the actual workhorse of the application.
///
/// This thing handles one request. The plumbing behind the scenes give it access to the relevant
/// parts of config.
#[allow(clippy::needless_pass_by_value)] // The server_configured expects this signature
fn hello(spirit: &Arc<Spirit<Opts, Cfg>>, cfg: &Arc<Server>, req: Request<Body>) -> Response<Body> {
    trace!("Handling request {:?}", req);
    // Get some global configuration
    let mut msg = format!("{}\n", spirit.config().ui.msg);
    // Get some listener-local configuration.
    if let Some(ref signature) = extract_signature(cfg) {
        msg.push_str(&format!("Brought to you by {}\n", signature));
    }
    Response::new(Body::from(msg))
}

/// Putting it all together and starting.
fn main() {
    Spirit::<Opts, Cfg>::new()
        // The baked in configuration.
        .config_defaults(DEFAULT_CONFIG)
        // In addition to specifying configuration in files and command line, also allow overriding
        // it through an environment variable. This is useful to passing secrets in many
        // deployments (like all these docker based clouds).
        .config_env("HELLO")
        // If passed a directory, look for files with these extensions and load them as
        // configurations.
        //
        // Note that if a file is added or removed at runtime and the application receives SIGHUP,
        // the change is reflected.
        .config_exts(&["toml", "ini", "json"])
        // Plug in the daemonization configuration and command line arguments. The library will
        // make it alive ‒ it'll do the actual daemonization based on the config, it only needs to
        // be told it should do so this way.
        .with(Daemon::extension(|cfg: &Cfg, opts: &Opts| {
            opts.daemon.transform(cfg.daemon.clone())
        }))
        // Similarly with logging.
        .with(
            Pipeline::new("logging").extract(|opts: &Opts, cfg: &Cfg| LogBoth {
                cfg: cfg.logging(),
                opts: opts.logging(),
            }),
        )
        // And with config help
        .with(CfgOpts::extension(Opts::cfg_opts))
        // A custom callback ‒ when a new config is loaded, we want to print it to logs.
        .on_config(|cmd_line, new_cfg| {
            debug!("Current cmdline: {:?} and config {:?}", cmd_line, new_cfg);
        })
        // Configure number of threads & similar
        .with(ThreadPoolConfig::extension(Cfg::threadpool))
        // If we didn't use ThreadPoolConfig, we would have to make sure we have tokio runtime (the
        // server pipeline would make sure it is available, but we plug it in quite late).
        //.with_singleton(Runtime::default())
        // And run the application.
        //
        // Mostly empty body here is fine. The rest of the work will happen afterwards, inside the
        // HTTP server.
        .run(|spirit| {
            let spirit_srv = Arc::clone(spirit);
            let build_server = move |builder: Builder<_>, cfg: &Server, _: &'static str| {
                let spirit = Arc::clone(&spirit_srv);
                let cfg = Arc::new(cfg.clone());
                builder.serve(move || {
                    let spirit = Arc::clone(&spirit);
                    let cfg = Arc::clone(&cfg);
                    service_fn_ok(move |req| hello(&spirit, &cfg, req))
                })
            };
            spirit.with(
                Pipeline::new("listen")
                    .extract_cfg(Cfg::listen)
                    .transform(BuildServer(build_server)),
            )?;
            Ok(())
        });
}
