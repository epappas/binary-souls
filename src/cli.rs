use clap::{Parser, Subcommand};
use libp2p::core::Multiaddr;
use std::path::PathBuf;

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
	#[clap(about = "Provide a file to the network")]
	Provide {
		#[arg(long, help = "Path to the file to provide")]
		path: PathBuf,
		#[arg(long, help = "Name of the file to provide")]
		name: String,
	},
	#[clap(about = "Get a file from the network")]
	Get {
		#[arg(long, help = "Name of the file to seek in the network")]
		name: String,
	},
}
