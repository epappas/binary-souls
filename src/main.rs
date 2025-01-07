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
		.with_env_filter(EnvFilter::from_default_env())
		.try_init();

	let cli = Cli::parse();

	let (mut network_client, mut network_events, peer_id, network_event_loop) =
		network::new(cli.secret_key_seed).await?;

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
		Commands::Provide { path, name } => {
			network_client.start_providing(name.clone()).await;

			loop {
				match network_events.next().await {
					Some(network::types::Event::InboundRequest { request, channel }) => {
						if request == name {
							network_client.respond_file(std::fs::read(&path)?, channel).await;
						}
					},
					e => todo!("{:?}", e),
				}
			}
		},
		Commands::Get { name } => {
			let providers = network_client.get_providers(name.clone()).await;
			if providers.is_empty() {
				return Err(format!("Could not find provider for file {name}.").into());
			}

			let requests = providers.into_iter().map(|p| {
				let mut network_client = network_client.clone();
				let name = name.clone();
				async move { network_client.request_file(p, name).await }.boxed()
			});

			let file_content = futures::future::select_ok(requests)
				.await
				.map_err(|_| "None of the providers returned file.")?
				.0;

			std::io::stdout().write_all(&file_content)?;
		},
	}

	Ok(())
}
