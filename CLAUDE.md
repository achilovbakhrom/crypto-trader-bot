t# Crypto Trader Bot — Claude Development Guide

## Project Overview

A Rust-based crypto trading bot implementing two strategies:
- **Liquidation Bot** — monitors DeFi lending protocols (Aave V3, Compound V3, Morpho) on Base L2, executes liquidations, sells collateral on Binance
- **Token Unlock Trading** — monitors vesting contracts, executes CEX short positions ahead of large unlocks

## Agent Workflow

When working on any non-trivial task:
1. Invoke `/pm` skill to break the task into subtasks
2. Run independent subtasks as parallel agents in a single message
3. Run dependent subtasks sequentially after their dependencies complete
4. Invoke `/reviewer` before creating any PR
5. Invoke `/rust-dev`, `/frontend-dev`, or `/db-reviewer` when reviewing domain-specific code

## Tech Stack

| Layer | Tool |
|---|---|
| Language | Rust (stable, latest) |
| Async runtime | Tokio |
| Blockchain | alloy-rs (NOT ethers-rs — deprecated) |
| CEX | Binance API (WebSocket + REST) |
| Database | PostgreSQL + sqlx (async) |
| TUI | ratatui |
| Web backend | Axum |
| Web frontend | React |
| Observability | OpenObserve (logs + metrics + traces) |
| Error handling | thiserror (libraries), anyhow (application) |
| Config | TOML + .env for secrets |

## Repository Structure

```
crypto-trader-bot/
├── CLAUDE.md
├── Cargo.toml              <- workspace root
├── config/
│   ├── default.toml        <- non-secret config
│   └── .env.example        <- template, never commit .env
├── crates/
│   ├── core/               <- shared types, errors, config
│   ├── feeds/              <- Binance WebSocket price feeds
│   ├── chain/              <- alloy: L2 RPC, contract reads/writes
│   ├── strategies/
│   │   ├── liquidation/
│   │   └── token_unlock/
│   ├── execution/          <- Binance order placement
│   ├── storage/            <- DB models, migrations, queries
│   ├── monitor/            <- tracing setup, OpenTelemetry export
│   └── ui/
│       ├── tui/            <- ratatui terminal UI
│       └── web/            <- Axum server + React frontend
└── src/
    └── main.rs
```

## Git Workflow

### Branch Model
```
main        <- production only, protected, never push directly
dev         <- default integration branch, base for all features
feature/*   <- branch off dev, PR back to dev
```

### Branch Naming
```
feature/<short-description>   <- new functionality
fix/<short-description>       <- bug fix
refactor/<short-description>  <- code restructure, no behavior change
hotfix/<short-description>    <- urgent production fix
```

### Commit Format
```
type(scope): short description

Examples:
feature(liquidation): add health factor monitor
fix(chain): handle RPC timeout on Base Sepolia
refactor(feeds): split binance ws into separate modules
```

### Rules
- One feature = one commit
- No co-author lines in commit messages
- Every feature goes through a PR to `dev` — no direct push to `dev` or `main`
- Run `/reviewer` skill before creating any PR
- PR must be approved before merge
- `main` is only updated from `dev` when releasing to production

### Pre-push Hook (automated)
All checks must pass before any push is accepted:
- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test`
- `prettier --check` (frontend, if directory exists)
- `eslint --max-warnings 0` (frontend, if directory exists)

A failed check blocks the push. Fix the issue, do not skip the hook.

## Code Standards

### Error Handling
- Never use `.unwrap()` or `.expect()` in production paths
- Use `?` operator for error propagation
- `thiserror` for errors defined in library crates
- `anyhow` for application-level error handling
- Tests may use `.unwrap()` freely

### Numeric Types
- Never use `f64`/`f32` for prices or amounts in critical paths
- Use integer arithmetic: wei, lamports, basis points
- Use newtype pattern for domain values:
  ```rust
  struct PriceWei(u128);
  struct AmountWei(u128);
  ```

### File Size
- No file should exceed ~300 lines
- Split by responsibility, not arbitrarily
- Each module has a single clear purpose

### Code Cleanliness
- No commented-out code in commits
- No unused imports or variables (treat warnings as errors)
- No magic numbers — use named constants
- No `.clone()` without a clear reason

### Dependencies
- Always use the latest stable version of each crate
- Check crates.io for the latest version before adding — do not guess version numbers
- Prefer actively maintained crates (recent releases, active repo)
- If a crate has not had a release in 6+ months, look for a maintained alternative
- Every new dependency needs justification — no heavy crates for small tasks
- Blockchain ecosystem moves fast: verify docs match the exact crate version in Cargo.toml
- Use `alloy` not `ethers-rs` (ethers-rs is deprecated and unmaintained)
- When integrating with protocols (Aave, Compound, etc.), always check their latest SDK/ABI — protocol contracts get upgraded

### Secrets
- API keys, private keys — only from `.env`
- `.env` is gitignored, never committed
- `.env.example` contains placeholder values as a template

## Database

- Migrations in `crates/storage/migrations/`
- Use `sqlx migrate` for all schema changes
- Never alter tables manually — always through migrations
- Keep tables normalized and clean
- Use explicit column lists in queries, never `SELECT *`

## Observability

- Structured logging via `tracing` crate with JSON format
- Export to OpenObserve via OpenTelemetry
- Log levels: ERROR (action needed), WARN (investigate), INFO (key events), DEBUG (verbose)
- Every trade execution logged with full context (symbol, size, price, exchange, strategy)

## Testing

- Unit tests for pure logic: health factor math, profit calculation, fee math
- Integration tests against Base Sepolia testnet — not mainnet
- No mocking the database in integration tests — use a test DB instance
- Test files inline with `#[cfg(test)]` for unit tests

## Network

- Base Sepolia testnet for development and testing
- Base mainnet for production
- RPC endpoint configured in `.env` (Alchemy or QuickNode)
- Liquidation mode: own capital on testnet, flash loans on mainnet

## Frontend

- React (latest stable) for admin panel — trade history, P&L charts, config
- ratatui for terminal UI — real-time positions, health factors, live event log
- Axum serves both the REST/WebSocket API and static React build
- Real-time data pushed via WebSocket from Axum to both TUI and React
