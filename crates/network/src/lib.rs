pub mod behaviour;
pub mod client;
pub mod eventloop;
pub mod types;

use std::{error::Error, time::Duration};

use futures::{channel::mpsc, prelude::*};
use libp2p::{identity, noise, tcp, tls, yamux};

pub use crate::behaviour::AsnBehaviour;
pub use crate::client::Client;
pub use crate::eventloop::EventLoop;
pub use crate::types::Event;

pub use libp2p::multiaddr::Protocol;
pub use libp2p::Multiaddr;

pub async fn new(
	secret_key_seed: Option<u8>,
	additional_topics: Vec<String>,
) -> Result<(Client, impl Stream<Item = Event>, libp2p::PeerId, EventLoop), Box<dyn Error>> {
	// Create a public/private key pair, either random or based on a seed.
	let id_key = match secret_key_seed {
		Some(seed) => {
			let mut bytes = [0u8; 32];
			bytes[0] = seed;
			identity::Keypair::ed25519_from_bytes(bytes).unwrap()
		},
		None => identity::Keypair::generate_ed25519(),
	};
	let peer_id = id_key.public().to_peer_id();

	let (command_sender, command_receiver) = mpsc::channel(0);
	let (event_sender, event_receiver) = mpsc::channel(0);

	let mut swarm = libp2p::SwarmBuilder::with_existing_identity(id_key)
		.with_tokio()
		.with_tcp(tcp::Config::default().nodelay(true), noise::Config::new, yamux::Config::default)?
		.with_quic()
		.with_dns()?
		.with_websocket((tls::Config::new, noise::Config::new), yamux::Config::default)
		.await?
		.with_behaviour(AsnBehaviour::new)?
		.with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
		.build();

	swarm.behaviour_mut().bootstrap();

	for topic in additional_topics {
		tracing::info!("Subscribed to topic: {topic}");
		swarm.behaviour_mut().subscribe(topic.as_str());
	}

	Ok((
		Client { sender: command_sender },
		event_receiver,
		peer_id,
		EventLoop::new(swarm, command_receiver, event_sender, None, None, None, None),
	))
}
