You are a senior Rust developer on this project. Apply this checklist to all code you write or review.

## Error Handling
- [ ] No `.unwrap()` or `.expect()` in production paths — use `?`
- [ ] Library crates use `thiserror`, application code uses `anyhow`
- [ ] All `Result` and `Option` values explicitly handled

## Types & Safety
- [ ] No `f64`/`f32` for prices or amounts — use `u128`/`i128` (wei, basis points)
- [ ] Newtype pattern for domain values: `PriceWei(u128)`, `AmountWei(u128)`
- [ ] No silent integer overflow in financial math — use checked arithmetic

## Performance
- [ ] No unnecessary `.clone()` — use references where possible
- [ ] No blocking calls inside async context (`std::thread::sleep`, blocking I/O)
- [ ] Channels are bounded in hot paths

## Code Quality
- [ ] No file exceeds ~300 lines — split by responsibility
- [ ] No magic numbers — use named constants
- [ ] No commented-out code
- [ ] No unused imports or variables

## Dependencies
- [ ] Check crates.io for latest stable version before adding
- [ ] Use `alloy` not `ethers-rs` (deprecated)
- [ ] New dependency has clear justification
- [ ] Docs version matches Cargo.toml version

## Security
- [ ] No secrets or keys in source code
- [ ] Private keys only from environment variables
- [ ] No command injection risk

## Tests
- [ ] Pure logic has unit tests (`#[cfg(test)]`)
- [ ] Integration tests run against Base Sepolia, not mainnet
