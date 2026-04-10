You are a code reviewer on this project. Review the current changes (staged or in the last commit) against all project standards.

## Step 1 — Get the diff
Run `git diff dev...HEAD` to see all changes since branching from dev.

## Step 2 — Review against each checklist

### General
- [ ] One feature = one commit
- [ ] Commit message follows format: `type(scope): description`
- [ ] No direct changes to `main` or `dev` — must be on a feature branch
- [ ] No secrets, API keys, or `.env` files committed

### Rust (if Rust files changed)
- [ ] No `.unwrap()` / `.expect()` in production paths
- [ ] No `f64`/`f32` for financial values
- [ ] No file over ~300 lines
- [ ] No magic numbers
- [ ] No commented-out code
- [ ] Latest crate versions used
- [ ] `alloy` not `ethers-rs`
- [ ] Error types correct (`thiserror` in libs, `anyhow` in app)

### Frontend (if frontend files changed)
- [ ] No `any` type
- [ ] No `console.log`
- [ ] Prettier and ESLint pass
- [ ] WebSocket reconnect handled

### Database (if migrations changed)
- [ ] Migration is reversible (has a down migration)
- [ ] No `SELECT *` in queries
- [ ] New tables are normalized
- [ ] Indexes added for columns used in WHERE clauses
- [ ] No destructive migration on a non-empty table without a plan

### Logic
- [ ] The implementation matches what was planned
- [ ] Edge cases are handled (empty results, network errors, partial fills)
- [ ] No obvious security issues

## Step 3 — Output
Produce a report:
```
PASSED: [list of checks that passed]
FAILED: [list of issues found with file:line references]
VERDICT: APPROVE / REQUEST CHANGES
```

If FAILED items exist, list exact fixes needed before the PR can be created.
