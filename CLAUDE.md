# CLAUDE.md - Guide for DASN (Decentralized Agentic Swarm Networks) Codebase

## Build & Test Commands
- Build: `make build` (debug) or `make build-release` (release)
- Test: `make test` (all) or `make run-test TEST=name` (specific)
- Debug test: `make debug TEST=name`
- Code quality: `make lint` (clippy), `make format` (rustfmt)
- Documentation: `make doc`
- Docker: `make docker-build`, `make docker-push`

## Code Style Guidelines
- **Formatting**: Hard tabs, 100 char line width, trailing commas (vertical)
- **Error handling**: Custom error types with From impls, thiserror/anyhow
- **Naming**: Snake_case for variables/functions, PascalCase for types
- **Architecture**: Workspace with multiple crates (ai-agent, network)
- **Imports**: Group by standard lib, external crates, then internal modules
- **Comments**: Document public APIs, 100 char width, wrapped comments
- **Testing**: Unit tests in same file as code, integration tests in tests/ dir

## Toolchain
- Rust stable channel with clippy, rust-analyzer, rustfmt
- Target: x86_64-unknown-linux-gnu