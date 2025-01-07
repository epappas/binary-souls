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

	let (mut network_client, mut network_events, network_event_loop) =
		network::new(cli.secret_key_seed).await?;

	// Spawn the network task for it to run in the background.
	spawn(network_event_loop.run());

	// In case a listen address was provided use it, otherwise listen on any
	// address.
	match cli.listen_address {
		Some(addr) => network_client.start_listening(addr).await.expect("Listening not to fail."),
		None => network_client
			.start_listening("/ip4/0.0.0.0/tcp/0".parse()?)
			.await
			.expect("Listening not to fail."),
	};

	// In case the user provided an address of a peer on the CLI, dial it.
	for addr in cli.peer {
		let Some(Protocol::P2p(peer_id)) = addr.iter().last() else {
			return Err("Expect peer multiaddr to contain peer ID.".into());
		};
		network_client.dial(peer_id, addr).await.expect("Dial to succeed");
	}

	match cli.command {
		// Providing a file.
		Commands::Provide { path, name } => {
			// Advertise oneself as a provider of the file on the DHT.
			network_client.start_providing(name.clone()).await;

			loop {
				match network_events.next().await {
					// Reply with the content of the file on incoming requests.
					Some(network::types::Event::InboundRequest { request, channel }) => {
						if request == name {
							network_client.respond_file(std::fs::read(&path)?, channel).await;
						}
					},
					e => todo!("{:?}", e),
				}
			}
		},
		// Locating and getting a file.
		Commands::Get { name } => {
			// Locate all nodes providing the file.
			let providers = network_client.get_providers(name.clone()).await;
			if providers.is_empty() {
				return Err(format!("Could not find provider for file {name}.").into());
			}

			// Request the content of the file from each node.
			let requests = providers.into_iter().map(|p| {
				let mut network_client = network_client.clone();
				let name = name.clone();
				async move { network_client.request_file(p, name).await }.boxed()
			});

			// Await the requests, ignore the remaining once a single one succeeds.
			let file_content = futures::future::select_ok(requests)
				.await
				.map_err(|_| "None of the providers returned file.")?
				.0;

			std::io::stdout().write_all(&file_content)?;
		},
	}

	Ok(())
}
