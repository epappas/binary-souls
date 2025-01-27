#![doc = include_str!("../README.md")]

#[cfg(not(debug_assertions))]
use human_panic::setup_panic;
use tokio_util::sync::CancellationToken;

#[cfg(debug_assertions)]
extern crate better_panic;

mod agent;
mod cli;

use std::{error::Error, io::Write, time::Duration};

use clap::Parser;
use futures::{prelude::*, StreamExt};
use network::Protocol;
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
		.with_line_number(true)
		.with_env_filter(EnvFilter::from_env("RUST_LOG"))
		.try_init();

	let cli = Cli::parse();

	let cancellation_token = CancellationToken::new();

	let (mut network_client, mut network_events, peer_id, network_event_loop) =
		network::new(cli.secret_key_seed, vec![]).await?;

	tracing::info!("Starting node...");
	tracing::info!("Node ID: {:?}", peer_id);

	// Spawn the network task for it to run in the background.
	spawn(network_event_loop.run(cancellation_token));

	for addr in cli.listen_address {
		network_client
			.start_listening(addr.clone())
			.await
			.expect("Listening not to fail.");
		tracing::info!("Listening on: {:?}", addr);
	}

	// In case the user provided an address of a peer on the CLI, dial it.
	for addr in cli.peer {
		let Some(Protocol::P2p(peer_id)) = addr.iter().last() else {
			return Err("Expect peer multiaddr to contain peer ID.".into());
		};
		network_client.dial(peer_id, addr).await.expect("Dial to succeed");
		tracing::info!("Dialed peer: {:?}", peer_id);
	}

	match cli.command {
		Commands::Bootstrap {} => {
			let mut discover_tick = tokio::time::interval(Duration::from_secs(30));

			loop {
				tokio::select! {
					_ = discover_tick.tick() => {
					},
				}
			}
		},
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
			network_client.bootstrap().await;
			network_client.start_providing(name.clone()).await;

			loop {
				match network_events.next().await {
					Some(network::types::Event::InboundRequest {
						agent_name,
						message,
						channel,
					}) => {
						tracing::info!("Received request for agent: {:?}", agent_name);
						if agent_name == name {
							let output = crate::agent::respond_llm(message).await?;

							network_client.respond_llm(output.as_bytes().to_vec(), channel).await;
						}
					},
					e => {
						tracing::info!("Unhandled event: {:?}", e);
					},
				}
			}
		},
		Commands::Llm { name, message } => {
			let providers = network_client.get_providers(name.clone()).await;
			if providers.is_empty() {
				return Err(format!("Could not find provider for agent {name}.").into());
			}

			tracing::info!("Requesting agent: {:?} from providers: {:?}", name, providers);

			let requests = providers.into_iter().map(|p| {
				let mut network_client = network_client.clone();
				let name = name.clone();
				let message = message.clone();
				async move { network_client.request_agent(p, name, message).await }.boxed()
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
