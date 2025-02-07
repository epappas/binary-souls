use std::{collections::HashSet, error::Error};
use thiserror::Error;

use futures::channel::oneshot;
use libp2p::{core::Multiaddr, request_response::ResponseChannel, PeerId};
use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub enum Command {
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
		agent_name: String,
		sender: oneshot::Sender<()>,
	},
	GetProviders {
		agent_name: String,
		sender: oneshot::Sender<HashSet<PeerId>>,
	},
	RequestAgent {
		agent_name: String,
		message: String,
		peer: PeerId,
		sender: oneshot::Sender<Result<Vec<u8>, Box<dyn Error + Send>>>,
	},
	RespondLLM {
		llm_output: Vec<u8>,
		channel: ResponseChannel<LLMResponse>,
	},
	GossipMessage {
		topic: String,
		message: String,
	},
}

#[derive(Debug)]
pub enum Event {
	LLMInboundRequest { agent_name: String, message: String, channel: ResponseChannel<LLMResponse> },
	InboundTaskProposal { task_proposal: TaskProposal },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LLMRequest(pub String, pub String);
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LLMResponse(pub Vec<u8>);

#[derive(Debug, Serialize, Deserialize)]
pub enum TaskType {
	ImageGeneration,
	DataProcessing,
	WebResearch,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TaskProposal {
	pub agent_name: String,
	pub task_id: String,
	pub task_type: TaskType,
	pub task_message: String,
	pub max_bid: f64,
	pub deadline: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BidResponse {
	pub task_id: String,
	pub capabilities: Vec<String>,
	pub bid: f64,
}

#[derive(Error, Debug)]
pub enum ProtocolError {
	#[error("Serialization error: {0}")]
	SerdeError(#[from] serde_json::Error),
	#[error("Invalid message format")]
	InvalidFormat,
}

pub fn serialize_message<T: Serialize>(msg: &T) -> Result<Vec<u8>, ProtocolError> {
	serde_json::to_vec(msg).map_err(Into::into)
}

pub fn deserialize_message<T: for<'a> Deserialize<'a>>(data: &[u8]) -> Result<T, ProtocolError> {
	serde_json::from_slice(data).map_err(Into::into)
}
