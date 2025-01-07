pub mod client;
pub mod eventloop;
pub mod types;

use std::{error::Error, time::Duration};

use futures::{channel::mpsc, prelude::*};
use libp2p::{
	identity, kad, noise,
	request_response::{self, ProtocolSupport},
	tcp, yamux, StreamProtocol,
};

use crate::network::client::Client;
use crate::network::eventloop::EventLoop;
use crate::network::types::{Behaviour, Event};

pub async fn new(
	secret_key_seed: Option<u8>,
) -> Result<(Client, impl Stream<Item = Event>, EventLoop), Box<dyn Error>> {
	// Create a public/private key pair, either random or based on a seed.
	let id_keys = match secret_key_seed {
		Some(seed) => {
			let mut bytes = [0u8; 32];
			bytes[0] = seed;
			identity::Keypair::ed25519_from_bytes(bytes).unwrap()
		},
		None => identity::Keypair::generate_ed25519(),
	};
	let peer_id = id_keys.public().to_peer_id();

	let mut swarm = libp2p::SwarmBuilder::with_existing_identity(id_keys)
		.with_tokio()
		.with_tcp(tcp::Config::default(), noise::Config::new, yamux::Config::default)?
		.with_behaviour(|key| Behaviour {
			kademlia: kad::Behaviour::new(
				peer_id,
				kad::store::MemoryStore::new(key.public().to_peer_id()),
			),
			request_response: request_response::cbor::Behaviour::new(
				[(StreamProtocol::new("/agentic-network/0.1.0"), ProtocolSupport::Full)],
				request_response::Config::default(),
			),
		})?
		.with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
		.build();

	swarm.behaviour_mut().kademlia.set_mode(Some(kad::Mode::Server));

	let (command_sender, command_receiver) = mpsc::channel(0);
	let (event_sender, event_receiver) = mpsc::channel(0);

	Ok((
		Client { sender: command_sender },
		event_receiver,
		EventLoop::new(swarm, command_receiver, event_sender),
	))
}
