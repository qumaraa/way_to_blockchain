/*
libp2p = { version = "0.53.2", features = ["full"] }
tokio = { version = "1.0", features = ["io-util", "io-std", "macros", "rt", "rt-multi-thread", "sync", "fs"] }
serde = {version = "1.0", features = ["derive"] }
serde_json = "1.0"
once_cell = "1.5"
log = "0.4"
pretty_env_logger = "0.4"
futures = "0.3.30"
tracing-subscriber = "0.3.18"
*/
use libp2p::{noise, ping, swarm::SwarmEvent, tcp, yamux, Multiaddr};
use log::{error, info};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::time::Duration;
use futures::StreamExt;
use tokio::{fs, io::AsyncBufReadExt, sync::mpsc};
use pretty_env_logger;
 

#[tokio::main]
async fn main() -> Result<(),Box<dyn std::error::Error>> {
    pretty_env_logger::init();
    let mut swarm = libp2p::SwarmBuilder::with_new_identity()
        .with_tokio()
        .with_tcp(
            tcp::Config::default(),
            noise::Config::new,
            yamux::Config::default,
        )?
        .with_behaviour(|_| ping::Behaviour::default())?
        .with_swarm_config(|cfg| cfg.with_idle_connection_timeout(Duration::from_secs(u64::MAX)))
        .build();

    swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

    if let Some(addr) = std::env::args().nth(1) {
        let remote: Multiaddr = addr.parse()?;
        swarm.dial(remote)?;
        info!("Dialed {addr}")
    }

    loop {
        match swarm.select_next_some().await {
            SwarmEvent::NewListenAddr {address, ..} => {
                info!("Listening on {address:?}")
            },
            SwarmEvent::Behaviour(event) => {
                info!("{event:?}")
            },
            _ => {}
        }
    }
}
