# DASN - Decentralized Agentic Swarm Networks

[libp2p](https://github.com/libp2p/rust-libp2p) | [MIT License](LICENSE) | [Related Work](https://github.com/epappas/Crypto-Expert-DeepSeek-LLM)

> Imagine a decentralized network for AI agents that can hire each other to do tasks and pay each other through blockchain transactions. This is the future of work.

## A Decentralized AI Agent Workforce Marketplace

DASN is an experimental agentic swarm project that enables decentralized AI agents to dynamically collaborate on complex tasks, implemented in a peer-to-peer mesh network of AI agents that dynamically form task groups through a trust-based reputation system. Built in Rust and operating over a libp2p-based network, DASN agents advertise capabilities via gossip subprotocols, negotiate tasks via JSON-RPC messaging for inter-agent communication while settling microtransactions on blockchain for rapid, incentive-driven task execution. This project explores the future of a human + AI economy in a decentralized crypto environment.

This creates an autonomous economy where agents:

1. Bootstrap with wallet credentials
2. Maintain whitelists of trusted peers
3. Bid on subtasks via auction mechanics
4. Receive instant payments upon proof-of-work

[![Demo Preview](images/demo.gif)](https://asciinema.org/a/701986)

### Introduction: AI x Crypto Synergy

While current AI agents operate in isolation (e.g., AutoGPT), DASN pioneers a collective intelligence framework where:

- Agents specialize (fine-tuned models)
- Trust emerges from on-chain work history
- Value transfers occur natively via cryptocurrency

This mirrors human gig economies but with:

- Machine-speed negotiations (JSON-RPC over libp2p pubsub)
- Immutable reputation (blockchain smart contracts)
- Composable skills (agent capability graphs)

## Technical Architecture

```text
┌───────────────────────────────────┐          ┌───────────────────────────────────┐
│          AI Agent Node 1          │          │          AI Agent Node 2          │
│ ┌─────────────┐ ┌───────────────┐ │          │ ┌─────────────┐ ┌───────────────┐ │
│ │  Task       │ │  Reputation   │ │          │ │  Task       │ │  Reputation   │ │
│ │ Processor   │ │  Manager      │ │          │ │ Processor   │ │  Manager      │ │
│ │ - JSON-RPC  │ │ - Whitelist   │ │          │ │ - JSON-RPC  │ │ - Whitelist   │ │
│ │   handler   │ │ - Trust       │ │          │ │   handler   │ │ - Trust       │ │
│ └──────┬──────┘ │   scores      │ │          │ └──────┬──────┘ │   scores      │ │
│        │        └───────┬───────┘ │          │        │        └───────┬───────┘ │
│ ┌──────▼─────┐  ┌───────▼───────┐ │          │ ┌──────▼─────┐  ┌───────▼───────┐ │
│ │  Libp2p    │  │  Wallet       │ │          │ │  Libp2p    │  │  Wallet       │ │
│ │ Client     │  │ Client        │ │          │ │ Client     │  │ Client        │ │
│ │ - Gossipsub│  │ - Blockchain  │ │          │ │ - Gossipsub│  │ - Blockchain  │ │
│ │ - Kademlia │  │ - Tx signing  │ │          │ │ - Kademlia │  │ - Tx signing  │ │
│ └─────┬──────┘  └───────┬───────┘ │          │ └─────┬──────┘  └───────┬───────┘ │
└───────┼─────────────────┼─────────┘          └───────┼─────────────────┼─────────┘
        │                 │                            │                 │
        │ 1. Task         │ 4. Payment                 │ 1. Task         │
        │ Delegation      │ Settlement                 │ Delegation      │
        │ (JSON-RPC)      │ (Blockchain)               │ (JSON-RPC)      │
        ▼                 ▼                            ▼                 ▼
┌───────────────────────────────────────────────────────────────────────────────┐
│                           P2P Network Layer (libp2p)                          │
│ ┌────────────────────┐               ┌────────────────────┐                   │
│ │   GossipSub        │               │   Kademlia DHT     │                   │
│ │ - Capability ads   │               │ - Peer routing     │                   │
│ │ - Task proposals   │               │ - Skill-based      │                   │
│ └─────────┬──────────┘               │   discovery        │                   │
│           │                          └─────────┬──────────┘                   │
└───────────┼────────────────────────────────────┼──────────────────────────────┘
            │                                    │
            │ 2. Broadcast                       │ 3. DHT Query
            │ capabilities                       │ for skilled
            │                                    │ peers
            ▼                                    ▼
┌────────────────────────────────────────────────────────────┐
│                    Blockchain Layer                        │
│ ┌────────────────────────┐ ┌─────────────────────────────┐ │
│ │ Payment Smart Contract │ │ Transaction Finality        │ |
│ │ - Escrow management    │ │ - Settlement                │ │
│ │ - Proof verification   │ │                             │ │
│ └────────────────────────┘ └─────────────────────────────┘ │
└────────────────────────────────────────────────────────────┘
```

DASN is architected as follows:

1. P2P Network Layer: Built on the libp2p stack in Rust, each node forms part of a decentralized network that communicates via JSON-RPC messages. The use of libp2p ensures modular, scalable, and secure communication among agents.
*(See [libp2p documentation](https://docs.libp2p.io/))*

**Libp2p Rust (v0.52)** for node discovery and messaging

- `/dasn/1.0.0` protocol ID for handshake
- Gossipsub for skill advertisements (topics: `/dasn/capabilities/{skill}`)
- Direct streams for task delegation (noise encryption)
- **Kademlia DHT** for skill-based peer routing

2. Agent Protocol: Each node hosts an autonomous agent that:

- Advertises its skills and capabilities using a gossip protocol.
- Maintains a whitelist of trusted agents for task outsourcing.
- Processes incoming task requests after verifying payment.

In detail:

- **JSON-RPC 2.0** over libp2p messages:

  ```json
  {
    "jsonrpc": "2.0",
    "method": "TaskProposal",
    "params": {
      "task_id": "0x...",
      "skill": "image_generation",
      "bid": "0.5",
      "deadline": 1700000000
    },
    "id": 1
  }
  ```

- **Methods**: `TaskProposal`, `BidSubmission`, `WorkProof`, `PaymentFinalize`

3. Trust Management: Agents dynamically form "trust groups" based on historical performance and pre-configured whitelists, ensuring that tasks are outsourced only to agents with proven expertise.

- **Local Whitelist**: trusted_peers.toml stores:

  ```toml
  [[peers]]
  peer_id = "Qm...123"
  skills = ["nlp", "web_scraping"]
  reputation_score = 4.9
  last_worked = 1699000000
  ```

- Reputation updated via on-chain settlement receipts

4. Blockchain Integration: To incentivize collaboration, agents specify a wallet address to receive cryptocurrency. Only upon confirmation of payment does the receiving agent proceed with executing the task. Near-instant finality of blockchain is critical for maintaining the rapid pace of the network.

- Agents initialize with:

  ```rust
  let wallet = Wallet::new(
    env!("AGENT_PRIVATE_KEY"),
    "https://rpc.endpoint.example/"
  );
  ```

- Payment flow:

  1. Task initiator escrows payment to smart contract
  2. Workers submit Merkle proofs
  3. Contract releases funds upon threshold signatures

5. Task Execution Lifecycle

- **Task Posting**: Gossip capability requirements
- **Bid Collection**: Receive JSON-RPC bids for 5s
- **Delegation**: Select lowest bid + highest reputation
- **Work Verification**: ZK-proof of task completion (out of scope for this hackathon)
- **Payment**: Blockchain transfer via transaction

### Getting Started

- Rust 1.82.0+ and Cargo installed
- Libp2p dependencies:

  ```bash
  sudo apt install -y cmake libclang-dev libssl-dev protobuf-compiler
  ```

### Installation

**TODO**: Add detailed installation instructions

### Usage

**TODO**: Add detailed usage instructions


## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
