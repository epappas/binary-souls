use std::{collections::HashSet, error::Error};

use futures::{
	channel::{mpsc, oneshot},
	prelude::*,
};
use libp2p::{core::Multiaddr, request_response::ResponseChannel, PeerId};

use crate::network::types::{Command, LLMResponse};

#[derive(Clone)]
pub(crate) struct Client {
	pub sender: mpsc::Sender<Command>,
}

impl Client {
	/// Listen for incoming connections on the given address.
	pub(crate) async fn start_listening(
		&mut self,
		addr: Multiaddr,
	) -> Result<(), Box<dyn Error + Send>> {
		let (sender, receiver) = oneshot::channel();
		self.sender
			.send(Command::StartListening { addr, sender })
			.await
			.expect("Command receiver not to be dropped.");
		receiver.await.expect("Sender not to be dropped.")
	}

	/// Dial the given peer at the given address.
	pub(crate) async fn dial(
		&mut self,
		peer_id: PeerId,
		peer_addr: Multiaddr,
	) -> Result<(), Box<dyn Error + Send>> {
		let (sender, receiver) = oneshot::channel();
		self.sender
			.send(Command::Dial { peer_id, peer_addr, sender })
			.await
			.expect("Command receiver not to be dropped.");
		receiver.await.expect("Sender not to be dropped.")
	}

	/// Advertise the local node as the provider of the given agent on the DHT.
	pub(crate) async fn start_providing(&mut self, agent_name: String) {
		let (sender, receiver) = oneshot::channel();
		self.sender
			.send(Command::StartProviding { agent_name, sender })
			.await
			.expect("Command receiver not to be dropped.");
		receiver.await.expect("Sender not to be dropped.");
	}

	/// Find the providers for the given file on the DHT.
	pub(crate) async fn get_providers(&mut self, agent_name: String) -> HashSet<PeerId> {
		let (sender, receiver) = oneshot::channel();
		self.sender
			.send(Command::GetProviders { agent_name, sender })
			.await
			.expect("Command receiver not to be dropped.");
		receiver.await.expect("Sender not to be dropped.")
	}

	/// Request the content of the given file from the given peer.
	pub(crate) async fn request_agent(
		&mut self,
		peer: PeerId,
		agent_name: String,
		message: String,
	) -> Result<Vec<u8>, Box<dyn Error + Send>> {
		let (sender, receiver) = oneshot::channel();
		self.sender
			.send(Command::RequestAgent { agent_name, message, peer, sender })
			.await
			.expect("Command receiver not to be dropped.");
		receiver.await.expect("Sender not be dropped.")
	}

	/// Respond with the provided llm output content to the given request.
	pub(crate) async fn respond_llm(
		&mut self,
		llm_output: Vec<u8>,
		channel: ResponseChannel<LLMResponse>,
	) {
		self.sender
			.send(Command::RespondLLM { llm_output, channel })
			.await
			.expect("Command receiver not to be dropped.");
	}

	/// Gossip the given message in the given topic.
	pub(crate) async fn gossip(
		&mut self,
		topic: String,
		message: String,
	) -> Result<(), Box<dyn Error + Send>> {
		self.sender
			.send(Command::GossipMessage { topic, message })
			.await
			.expect("Command receiver not to be dropped.");
		Ok(())
	}
}
