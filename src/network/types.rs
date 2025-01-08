use std::{collections::HashSet, error::Error};

use futures::channel::oneshot;
use libp2p::{
	autonat,
	core::Multiaddr,
	gossipsub, identify, kad, mdns, ping, relay, rendezvous,
	request_response::{self, ResponseChannel},
	swarm::NetworkBehaviour,
	upnp, PeerId,
};
use serde::{Deserialize, Serialize};

#[derive(NetworkBehaviour)]
pub(crate) struct Behaviour {
	pub identify: identify::Behaviour,
	pub request_response: request_response::cbor::Behaviour<FileRequest, FileResponse>,
	pub rendezvous: rendezvous::client::Behaviour,
	pub relay: relay::Behaviour,
	pub ping: ping::Behaviour,
	pub kademlia: kad::Behaviour<kad::store::MemoryStore>,
	pub auto_nat: autonat::Behaviour,
	pub mdns: mdns::tokio::Behaviour,
	pub gossipsub: gossipsub::Behaviour,
	pub upnp: upnp::tokio::Behaviour,
}

#[derive(Debug)]
pub(crate) enum Command {
	StartListening {
		addr: Multiaddr,
		sender: oneshot::Sender<Result<(), Box<dyn Error + Send>>>,
	},
	Dial {
		peer_id: PeerId,
		peer_addr: Multiaddr,
		sender: oneshot::Sender<Result<(), Box<dyn Error + Send>>>,
	},
	StartProviding {
		file_name: String,
		sender: oneshot::Sender<()>,
	},
	GetProviders {
		file_name: String,
		sender: oneshot::Sender<HashSet<PeerId>>,
	},
	RequestFile {
		file_name: String,
		peer: PeerId,
		sender: oneshot::Sender<Result<Vec<u8>, Box<dyn Error + Send>>>,
	},
	RespondFile {
		file: Vec<u8>,
		channel: ResponseChannel<FileResponse>,
	},
}

#[derive(Debug)]
pub(crate) enum Event {
	InboundRequest { request: String, channel: ResponseChannel<FileResponse> },
}

// Simple file exchange protocol
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct FileRequest(pub String);
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct FileResponse(pub Vec<u8>);
