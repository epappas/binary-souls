# DASN Architecture

DASN (Decentralized Agentic Swarm Networks) is a peer-to-peer network of AI agents that can collaborate on tasks through a decentralized protocol. This document outlines the architecture, components, and design principles.

## System Overview

DASN is built on three core layers:

1. **P2P Network Layer**: Built on libp2p for decentralized communication
2. **Agent Protocol Layer**: Defines agent interactions, task delegation, and trust
3. **AI/Task Execution Layer**: Handles agent capabilities and task processing

![Architecture Diagram](images/logo.png)

## Component Structure

The codebase is organized into a workspace with multiple crates:

### Core Application (`src/`)

- `main.rs`: Entry point, initializes the application components
- `cli.rs`: Command-line interface definition using Clap
- `agent.rs`: Base implementation of agent behavior

### Network Crate (`crates/network/`)

- `lib.rs`: Exports network components
- `behaviour.rs`: Configures and manages libp2p network behaviors
- `client.rs`: Client interface for network operations
- `eventloop.rs`: Event processing loop for network communications
- `types.rs`: Data structures for network protocol messages

### AI Agent Crate (`crates/ai-agent/`)

- `lib.rs`: Central agent functionality
- `oa_client.rs`: OpenAI API client wrapper
- `conv.rs`: Conversation management
- `chat.rs`: Message formatting
- `gpts.rs`: GPT model configuration
- `model.rs`: Model management
- `error.rs`: Error types for the AI agent
- `tools/`: Agent tool implementations

## Communication Protocol

Agents communicate using:

- **JSON-RPC 2.0** over libp2p for method invocation
- **Gossipsub** for capability advertisements and broadcast messages
- **Kademlia DHT** for skill-based peer discovery
- **Request/Response** pattern for direct agent communication

### Protocol Flow

1. Agents advertise capabilities via gossip protocol
2. Task initiators query the DHT for agents with specific skills
3. Task proposals are sent via Request/Response
4. Agents can bid on tasks they can fulfill
5. Task execution occurs after negotiation
6. Results and proofs are verified

## Key Design Patterns

DASN implements several architectural patterns:

- **Actor Pattern**: Components communicate via message passing
- **Event-driven Architecture**: Network events drive system behavior
- **Repository Pattern**: For capability discovery
- **Tool-based Extensibility**: Agent capabilities are modular tools

## Error Handling

The system uses:
- Custom error types with thiserror
- Result type aliases for consistent error handling
- Error propagation with '?' operator
- From trait implementations for error conversion

## Security & Trust

Security is ensured through:
- Ed25519 keypairs for peer identity
- Secure transport with Noise protocol
- Message signing for gossipsub
- Trust score tracking for peer reputation
- Whitelist management for trusted peers

## AI Integration

AI agents are integrated via:
- OpenAI API client
- Tool-based interaction pattern
- JSON schema for function calling
- Concurrent tool execution
- Conversation state management

## Configuration Management

The system uses:
- Environment variables for API credentials
- Command-line arguments for runtime configuration
- Network parameter constants
- Model configuration constants

## Future Extensions

The architecture supports planned extensions:
- Blockchain integration for payments
- Enhanced reputation management
- More sophisticated task delegation
- Additional agent capabilities through new tools

## Testing Strategy

Testing follows these patterns:
- Unit tests for specific components
- Custom test modules with error types
- Runtime verification of protocol operations

---

This architecture document provides a high-level overview of the DASN system design. For more detailed implementation information, refer to the inline code documentation.
