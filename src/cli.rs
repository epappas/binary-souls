use clap::{Parser, Subcommand};
use network::Multiaddr;

#[derive(Parser, Debug)]
#[command(
    name = "dasn",
    author = "Evangelos Pappas <epappas@evalonlabs.com>",
    about = "Decentralized Agentic Swarm Networks",
    long_about = "Decentralized Agentic Swarm Networks - A protocol for AI agents to collaborate",
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

	#[arg(
		long,
		short = 'p',
		value_name = "PEER",
		help = "Multiaddress of a peer to connect to  (can be multiple)"
	)]
	pub peer: Vec<Multiaddr>,

	#[arg(
		long,
		short = 'l',
		value_name = "LISTEN_ADDRESS",
		help = "Multiaddress to listen on (can be multiple)"
	)]
	pub listen_address: Vec<Multiaddr>,

	#[clap(subcommand)]
	pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
	#[clap(about = "Run a simple node just to bootstrap the network")]
	Bootstrap {},
	#[clap(about = "Provide a an AI Agent to the network")]
	Provide {
		#[arg(long, help = "Name of the Agent to provide")]
		name: String,
	},
	#[clap(about = "request LLM content from an agent in the network")]
	Llm {
		#[arg(long, help = "Name of the agent to seek in the network")]
		name: String,
		#[arg(long, help = "Message to send to the agent")]
		message: String,
	},
	#[clap(about = "Gossip a message in the network")]
	Gossip {
		#[arg(long, help = "Topic to publish the message in")]
		topic: String,
		#[arg(long, help = "Message to publish")]
		message: String,
	},
}
