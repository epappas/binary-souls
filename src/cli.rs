use clap::{Parser, Subcommand};
use libp2p::core::Multiaddr;

#[derive(Parser, Debug)]
#[command(
    name = "binary-souls",
    author = "Binary-souls Engineering <engineers@spacejar.io>",
    about = "Agentic p2p network.",
    long_about = "Agentic p2p network.",
    version = env!("CARGO_PKG_VERSION"),
)]
pub struct Cli {
	#[arg(
		long,
		short = 's',
		value_name = "SECRET_KEY_SEED",
		help = "Secret key seed for the node"
	)]
	pub secret_key_seed: Option<u8>,

	#[arg(long, short = 'p', value_name = "PEER", help = "Multiaddress of a peer to connect to")]
	pub peer: Vec<Multiaddr>,

	#[arg(long, short = 'l', value_name = "LISTEN_ADDRESS", help = "Multiaddress to listen on")]
	pub listen_address: Option<Multiaddr>,

	#[clap(subcommand)]
	pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
	#[clap(about = "Provide a an Ai Agent to the network")]
	Provide {
		#[arg(long, help = "Name of the Agent to provide")]
		name: String,
	},
	#[clap(about = "request LLM content froman agent in the network")]
	Llm {
		#[arg(long, help = "Name of the agent to seek in the network")]
		name: String,
	},
	#[clap(about = "Gossip a message in the network")]
	Gossip {
		#[arg(long, help = "Topic to publish the message in")]
		topic: String,
		#[arg(long, help = "Message to publish")]
		message: String,
	},
}
