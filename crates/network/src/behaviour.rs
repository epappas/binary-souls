use crate::types::{LLMRequest, LLMResponse};
use libp2p::{
	autonat, gossipsub, identify, identity, kad,
	kad::Config as KademliaConfig,
	mdns, ping, relay, rendezvous,
	request_response::{self, ProtocolSupport},
	swarm::NetworkBehaviour,
	upnp, PeerId, StreamProtocol,
};
use std::{
	collections::hash_map::DefaultHasher,
	hash::{Hash, Hasher},
	time::Duration,
};

static PROTOCOL_VERSION: &str = "/asn/1.0.0";
static EVERYONE_TOPIC: &str = "everyone";
static CAPABILITIES_TOPIC: &str = "capabilities";

#[derive(NetworkBehaviour)]
pub struct AsnBehaviour {
	pub identify: identify::Behaviour,
	pub request_response: request_response::cbor::Behaviour<LLMRequest, LLMResponse>,
	pub rendezvous: rendezvous::client::Behaviour,
	pub relay: relay::Behaviour,
	pub ping: ping::Behaviour,
	pub kademlia: kad::Behaviour<kad::store::MemoryStore>,
	pub auto_nat: autonat::Behaviour,
	pub mdns: mdns::tokio::Behaviour,
	pub gossipsub: gossipsub::Behaviour,
	pub upnp: upnp::tokio::Behaviour,
}

impl AsnBehaviour {
	pub fn new(key: &identity::Keypair) -> Self {
		let peer_id = key.public().to_peer_id();
		let mut kademlia_config = KademliaConfig::default();
		kademlia_config.set_provider_publication_interval(Some(Duration::from_secs(60)));

		Self {
			identify: identify::Behaviour::new(identify::Config::new(
				PROTOCOL_VERSION.into(),
				key.public().clone(),
			)),
			kademlia: kad::Behaviour::with_config(
				peer_id,
				kad::store::MemoryStore::new(peer_id),
				kademlia_config,
			),
			request_response: request_response::cbor::Behaviour::new(
				[(StreamProtocol::new(PROTOCOL_VERSION), ProtocolSupport::Full)],
				request_response::Config::default(),
			),
			rendezvous: rendezvous::client::Behaviour::new(key.clone()),
			relay: relay::Behaviour::new(key.public().to_peer_id(), Default::default()),
			ping: ping::Behaviour::new(
				ping::Config::new()
					.with_interval(Duration::from_secs(5))
					.with_timeout(Duration::from_secs(5)),
			),
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
					.heartbeat_interval(Duration::from_secs(10))
					.validation_mode(gossipsub::ValidationMode::Permissive)
					.allow_self_origin(true)
					.history_length(10)
					.history_gossip(10)
					.mesh_n_high(8)
					.mesh_n(6)
					.mesh_n_low(4)
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
		}
	}

	pub fn shutdown(&mut self) {
		self.kademlia.set_mode(None);
		self.gossipsub.unsubscribe(&gossipsub::IdentTopic::new(EVERYONE_TOPIC));
		self.gossipsub.unsubscribe(&gossipsub::IdentTopic::new(CAPABILITIES_TOPIC));
	}

	pub fn bootstrap(&mut self) {
		self.kademlia.set_mode(Some(kad::Mode::Server));
		self.kademlia.add_address(
			&PeerId::from(identity::Keypair::generate_ed25519().public()),
			"/ip4/0.0.0.0/tcp/0".parse().unwrap(),
		);

		tracing::info!("Subscribed to topic: {EVERYONE_TOPIC}");
		self.gossipsub.subscribe(&gossipsub::IdentTopic::new(EVERYONE_TOPIC)).unwrap();

		tracing::info!("Subscribed to topic: {CAPABILITIES_TOPIC}");
		self.gossipsub
			.subscribe(&gossipsub::IdentTopic::new(CAPABILITIES_TOPIC))
			.unwrap();

		match self.kademlia.bootstrap() {
			Ok(_) => {
				tracing::info!("Successfully bootstrapped");
			},
			Err(e) => {
				tracing::error!("Failed to bootstrap: {:?}", e);
			},
		}
	}

	pub fn subscribe(&mut self, topic: &str) {
		tracing::info!("Subscribed to topic: {topic}");
		self.gossipsub.subscribe(&gossipsub::IdentTopic::new(topic)).unwrap();
	}
}
