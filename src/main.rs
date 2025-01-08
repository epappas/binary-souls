#![doc = include_str!("../README.md")]

#[cfg(not(debug_assertions))]
use human_panic::setup_panic;

#[cfg(debug_assertions)]
extern crate better_panic;

mod cli;
mod network;

use std::{error::Error, io::Write};

use clap::Parser;
use futures::{prelude::*, StreamExt};
use libp2p::multiaddr::Protocol;
use tokio::task::spawn;
use tracing_subscriber::EnvFilter;

use cli::{Cli, Commands};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
	#[cfg(not(debug_assertions))]
	{
		setup_panic!();
	}

	#[cfg(debug_assertions)]
	{
		better_panic::Settings::debug()
			.most_recent_first(false)
			.lineno_suffix(true)
			.verbosity(better_panic::Verbosity::Full)
			.install();
	}

	let _ = tracing_subscriber::fmt()
		.with_level(true)
		.with_env_filter(EnvFilter::from_env("RUST_LOG"))
		.try_init();

	let cli = Cli::parse();

	let (mut network_client, mut network_events, peer_id, network_event_loop) =
		network::new(cli.secret_key_seed, vec![]).await?;

	tracing::info!("Starting node...");
	tracing::info!("Node ID: {:?}", peer_id);

	// Spawn the network task for it to run in the background.
	spawn(network_event_loop.run());

	if let Some(listen_address) = cli.listen_address {
		network_client
			.start_listening(listen_address.clone())
			.await
			.expect("Listening not to fail.");
		tracing::info!("Listening on: {:?}", listen_address);
	}

	// In case the user provided an address of a peer on the CLI, dial it.
	for addr in cli.peer {
		let Some(Protocol::P2p(peer_id)) = addr.iter().last() else {
			return Err("Expect peer multiaddr to contain peer ID.".into());
		};
		network_client.dial(peer_id, addr).await.expect("Dial to succeed");
		tracing::debug!("Dialed peer: {:?}", peer_id);
	}

	match cli.command {
		Commands::Gossip { topic, message } => {
			tracing::info!("Gossiping message: [{topic}] {message}");
			match network_client.gossip(topic, message).await {
				Ok(()) => {
					tracing::info!("Gossip done.");
				},
				Err(e) => tracing::error!("Failed to gossip message: {:?}", e),
			}
		},
		Commands::Provide { name } => {
			network_client.start_providing(name.clone()).await;

			loop {
				match network_events.next().await {
					Some(network::types::Event::InboundRequest { request, channel }) => {
						if request == name {
							network_client
								.respond_llm("Hello from Agent".as_bytes().to_vec(), channel)
								.await;
						}
					},
					e => todo!("{:?}", e),
				}
			}
		},
		Commands::Llm { name } => {
			let providers = network_client.get_providers(name.clone()).await;
			if providers.is_empty() {
				return Err(format!("Could not find provider for agent {name}.").into());
			}

			let requests = providers.into_iter().map(|p| {
				let mut network_client = network_client.clone();
				let name = name.clone();
				async move { network_client.request_agent(p, name).await }.boxed()
			});

			let agent_content = futures::future::select_ok(requests)
				.await
				.map_err(|_| "None of the providers returned agent.")?
				.0;

			std::io::stdout().write_all(&agent_content)?;
		},
	}

	Ok(())
}
