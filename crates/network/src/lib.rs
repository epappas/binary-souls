pub mod client;
pub mod eventloop;
pub mod types;

use std::{
	collections::hash_map::DefaultHasher,
	error::Error,
	hash::{Hash, Hasher},
	time::Duration,
};

use futures::{channel::mpsc, prelude::*};
use libp2p::{
	autonat, gossipsub, identify, identity, kad, mdns, noise, ping, relay, rendezvous,
	request_response::{self, ProtocolSupport},
	tcp, tls, upnp, yamux, StreamProtocol,
};

pub use crate::client::Client;
pub use crate::eventloop::EventLoop;
pub use crate::types::{Behaviour, Event};

pub use libp2p::multiaddr::Protocol;
pub use libp2p::Multiaddr;

static PROTOCOL_VERSION: &str = "/agentic-network/1.0.0";
static EVERYONE_TOPIC: &str = "everyone";

pub async fn new(
	secret_key_seed: Option<u8>,
	additional_topics: Vec<String>,
) -> Result<(Client, impl Stream<Item = Event>, libp2p::PeerId, EventLoop), Box<dyn Error>> {
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
	let peer_id_topic = gossipsub::IdentTopic::new(peer_id.to_base58());

	let mut swarm = libp2p::SwarmBuilder::with_existing_identity(id_keys)
		.with_tokio()
		.with_tcp(tcp::Config::default(), noise::Config::new, yamux::Config::default)?
		.with_quic()
		.with_dns()?
		.with_websocket((tls::Config::new, noise::Config::new), yamux::Config::default)
		.await?
		.with_behaviour(|key| Behaviour {
			identify: identify::Behaviour::new(identify::Config::new(
				PROTOCOL_VERSION.into(),
				key.public().clone(),
			)),
			kademlia: kad::Behaviour::new(
				peer_id,
				kad::store::MemoryStore::new(key.public().to_peer_id()),
			),
			request_response: request_response::cbor::Behaviour::new(
				[(StreamProtocol::new(PROTOCOL_VERSION), ProtocolSupport::Full)],
				request_response::Config::default(),
			),
			rendezvous: rendezvous::client::Behaviour::new(key.clone()),
			relay: relay::Behaviour::new(key.public().to_peer_id(), Default::default()),
			ping: ping::Behaviour::new(ping::Config::new()),
			upnp: upnp::tokio::Behaviour::default(),
			auto_nat: autonat::Behaviour::new(
				key.public().to_peer_id(),
				autonat::Config { only_global_ips: false, ..Default::default() },
			),
			mdns: mdns::tokio::Behaviour::new(mdns::Config::default(), key.public().to_peer_id())
				.unwrap(),
			gossipsub: gossipsub::Behaviour::new(
				gossipsub::MessageAuthenticity::Signed(key.clone()),
				gossipsub::ConfigBuilder::default()
					.heartbeat_interval(Duration::from_secs(10)) // This is set to aid debugging by not cluttering the log space
					.validation_mode(gossipsub::ValidationMode::Strict) // This sets the kind of message validation. The default is Strict (enforce message
					.allow_self_origin(true)
					.history_length(10)
					.history_gossip(10)
					.max_transmit_size(1024 * 1024 * 10)
					.message_id_fn(|message: &gossipsub::Message| {
						let mut s = DefaultHasher::new();
						message.data.hash(&mut s);
						gossipsub::MessageId::from(s.finish().to_string())
					})
					.build()
					.unwrap(),
			)
			.unwrap(),
		})?
		.with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
		.build();

	swarm.behaviour_mut().kademlia.set_mode(Some(kad::Mode::Server));

	tracing::trace!("Subscribed to topic: {EVERYONE_TOPIC}");
	swarm
		.behaviour_mut()
		.gossipsub
		.subscribe(&gossipsub::IdentTopic::new(EVERYONE_TOPIC))
		.unwrap();

	tracing::trace!("Subscribed to topic: {peer_id_topic}");
	swarm.behaviour_mut().gossipsub.subscribe(&peer_id_topic).unwrap();

	for topic in additional_topics {
		swarm
			.behaviour_mut()
			.gossipsub
			.subscribe(&gossipsub::IdentTopic::new(topic))
			.unwrap();
	}

	let (command_sender, command_receiver) = mpsc::channel(0);
	let (event_sender, event_receiver) = mpsc::channel(0);

	Ok((
		Client { sender: command_sender },
		event_receiver,
		peer_id,
		EventLoop::new(swarm, command_receiver, event_sender, None, None, None, None),
	))
}
