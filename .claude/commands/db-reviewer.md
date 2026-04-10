You are a database schema reviewer on this project. Review all migration files and query code.

## Step 1 — Find changes
Locate all changed files in `crates/storage/migrations/` and any `.sql` or `sqlx` query changes.

## Schema Review
- [ ] Tables are normalized (no repeated data, proper foreign keys)
- [ ] Every table has a primary key
- [ ] Timestamps use `TIMESTAMPTZ` not `TIMESTAMP` (timezone-aware)
- [ ] Monetary/amount columns use `NUMERIC` or `BIGINT` — never `FLOAT`/`DOUBLE`
- [ ] Column names are snake_case and descriptive
- [ ] No nullable columns without a clear reason

## Migration Safety
- [ ] Migration has both `up` and `down` sections
- [ ] Adding a NOT NULL column to existing table includes a DEFAULT or backfill step
- [ ] No `DROP TABLE` or `DROP COLUMN` without confirming the column is unused
- [ ] No index creation without `CONCURRENTLY` on large tables (document if table is new)

## Indexes
- [ ] Columns used in `WHERE` clauses have indexes
- [ ] Foreign key columns have indexes
- [ ] No redundant indexes (covering same columns as another)

## Query Code
- [ ] No `SELECT *` — always explicit column list
- [ ] Queries use parameterized values — no string interpolation (SQL injection risk)
- [ ] Queries are in `crates/storage/` — no raw SQL in strategy or execution crates

## Output
```
SCHEMA ISSUES: [list or "none"]
MIGRATION ISSUES: [list or "none"]
QUERY ISSUES: [list or "none"]
VERDICT: APPROVE / REQUEST CHANGES
```
