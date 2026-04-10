# ─────────────────────────────────────────────────────────────────────────────
# Crypto Trader Bot — Makefile
#
# Usage: make <target>
# Run `make help` to see all available targets.
# ─────────────────────────────────────────────────────────────────────────────

# Load local env (never committed). Silently skip if not present.
-include .env.local
export

# ── Project config ────────────────────────────────────────────────────────────
APP_NAME        := crypto-trader-bot
BINARY          := bot
DOCKER_REGISTRY := ghcr.io/achilovbakhrom
DOCKER_IMAGE    := $(DOCKER_REGISTRY)/$(APP_NAME)
IMAGE_TAG       := $(shell git rev-parse --short HEAD 2>/dev/null || echo "dev")
FRONTEND_DIR    := crates/ui/web/frontend
MIGRATIONS_DIR  := crates/storage/migrations

# ── Server config (set via .env.local or environment) ────────────────────────
SSH_USER        ?= root
SSH_HOST        ?= $(HETZNER_HOST)
DEPLOY_DIR      ?= /opt/crypto-trader-bot

# ── Colours ───────────────────────────────────────────────────────────────────
BOLD  := \033[1m
GREEN := \033[32m
CYAN  := \033[36m
RESET := \033[0m

.DEFAULT_GOAL := help
.PHONY: help setup check build build-release test lint fmt fmt-check \
        migrate migrate-revert migrate-create migrate-status \
        infra-up infra-down infra-restart infra-logs infra-status db-shell db-reset \
        frontend-install frontend-build frontend-dev frontend-lint \
        frontend-fmt frontend-fmt-check \
        docker-build docker-push docker-run docker-stop \
        deploy deploy-migrate logs ssh server-status \
        clean all-checks

# ─────────────────────────────────────────────────────────────────────────────
# Help
# ─────────────────────────────────────────────────────────────────────────────

help:
	@echo ""
	@echo "$(BOLD)$(CYAN)Crypto Trader Bot$(RESET)"
	@echo ""
	@echo "$(BOLD)Setup$(RESET)"
	@echo "  make setup              First-time project setup"
	@echo ""
	@echo "$(BOLD)Development$(RESET)"
	@echo "  make run                Run the bot locally"
	@echo "  make check              Cargo check (fast, no binary)"
	@echo "  make build              Cargo build (debug)"
	@echo "  make build-release      Cargo build (optimised release)"
	@echo ""
	@echo "$(BOLD)Code Quality$(RESET)"
	@echo "  make lint               Run cargo clippy (-D warnings)"
	@echo "  make fmt                Auto-format all Rust code"
	@echo "  make fmt-check          Check formatting without changes"
	@echo "  make test               Run all unit tests"
	@echo "  make all-checks         fmt-check + lint + test (same as CI)"
	@echo ""
	@echo "$(BOLD)Database$(RESET)"
	@echo "  make migrate            Run pending migrations"
	@echo "  make migrate-revert     Revert last migration"
	@echo "  make migrate-create name=<name>  Create new migration file"
	@echo "  make migrate-status     Show migration status"
	@echo "  make db-shell           Open psql shell"
	@echo "  make db-reset           Drop + recreate DB + run migrations"
	@echo ""
	@echo "$(BOLD)Infrastructure (local)$(RESET)"
	@echo "  make infra-up           Start PostgreSQL + OpenObserve"
	@echo "  make infra-down         Stop infrastructure"
	@echo "  make infra-restart      Restart infrastructure"
	@echo "  make infra-logs         Tail infrastructure logs"
	@echo "  make infra-status       Show container status"
	@echo ""
	@echo "$(BOLD)Frontend$(RESET)"
	@echo "  make frontend-install   npm install"
	@echo "  make frontend-build     Build React admin panel"
	@echo "  make frontend-dev       Start React dev server"
	@echo "  make frontend-lint      Run ESLint"
	@echo "  make frontend-fmt       Run Prettier (auto-fix)"
	@echo "  make frontend-fmt-check Check Prettier without changes"
	@echo ""
	@echo "$(BOLD)Docker$(RESET)"
	@echo "  make docker-build       Build production Docker image"
	@echo "  make docker-push        Push image to GHCR"
	@echo "  make docker-run         Run bot in Docker locally"
	@echo "  make docker-stop        Stop local Docker bot"
	@echo ""
	@echo "$(BOLD)Deploy$(RESET)"
	@echo "  make deploy             Build + push + deploy to Hetzner"
	@echo "  make deploy-migrate     Run migrations on prod server"
	@echo "  make logs               Tail prod logs via SSH"
	@echo "  make ssh                SSH into prod server"
	@echo "  make server-status      Show prod container status"
	@echo ""
	@echo "$(BOLD)Misc$(RESET)"
	@echo "  make clean              Remove build artifacts"
	@echo ""

# ─────────────────────────────────────────────────────────────────────────────
# Setup
# ─────────────────────────────────────────────────────────────────────────────

setup:
	@echo "$(BOLD)$(CYAN)Setting up project...$(RESET)"
	@if [ ! -f .env.local ]; then \
		cp config/.env.example .env.local; \
		echo "$(GREEN)Created .env.local from example. Fill in your credentials.$(RESET)"; \
	else \
		echo ".env.local already exists — skipping"; \
	fi
	@echo "Checking required tools..."
	@command -v docker    >/dev/null 2>&1 || (echo "ERROR: docker not installed"    && exit 1)
	@command -v cargo     >/dev/null 2>&1 || (echo "ERROR: cargo not installed"     && exit 1)
	@command -v sqlx      >/dev/null 2>&1 || (echo "Installing sqlx-cli..." && cargo install sqlx-cli --no-default-features --features postgres,native-tls)
	@echo "Starting local infrastructure..."
	@$(MAKE) infra-up
	@echo "Waiting for PostgreSQL to be ready..."
	@$(MAKE) _wait-for-postgres
	@echo "Running migrations..."
	@$(MAKE) migrate
	@echo ""
	@echo "$(GREEN)$(BOLD)Setup complete!$(RESET)"
	@echo "Edit .env.local with your API keys, then run: make run"

# ─────────────────────────────────────────────────────────────────────────────
# Development
# ─────────────────────────────────────────────────────────────────────────────

run:
	RUST_LOG=$(or $(RUST_LOG),info) cargo run --bin $(BINARY)

check:
	cargo check

build:
	cargo build

build-release:
	cargo build --release

# ─────────────────────────────────────────────────────────────────────────────
# Code Quality
# ─────────────────────────────────────────────────────────────────────────────

lint:
	cargo clippy -- -D warnings

fmt:
	cargo fmt

fmt-check:
	cargo fmt --check

test:
	cargo test

all-checks: fmt-check lint test
	@echo "$(GREEN)All checks passed.$(RESET)"

# ─────────────────────────────────────────────────────────────────────────────
# Database / Migrations
# ─────────────────────────────────────────────────────────────────────────────

migrate:
	sqlx migrate run --source $(MIGRATIONS_DIR) --database-url "$(DATABASE_URL)"

migrate-revert:
	sqlx migrate revert --source $(MIGRATIONS_DIR) --database-url "$(DATABASE_URL)"

migrate-create:
	@test -n "$(name)" || (echo "Usage: make migrate-create name=<migration_name>" && exit 1)
	sqlx migrate add --source $(MIGRATIONS_DIR) $(name)

migrate-status:
	sqlx migrate info --source $(MIGRATIONS_DIR) --database-url "$(DATABASE_URL)"

db-shell:
	@echo "Connecting to $(DATABASE_URL)..."
	psql "$(DATABASE_URL)"

db-reset:
	@echo "$(BOLD)Resetting database...$(RESET)"
	psql "$(DATABASE_URL)" -c "DROP SCHEMA public CASCADE; CREATE SCHEMA public;"
	$(MAKE) migrate
	@echo "$(GREEN)Database reset complete.$(RESET)"

# ─────────────────────────────────────────────────────────────────────────────
# Infrastructure (local Docker Compose)
# ─────────────────────────────────────────────────────────────────────────────

infra-up:
	docker compose -f docker-compose.yml up -d
	@echo "$(GREEN)Infrastructure started.$(RESET)"
	@echo "  PostgreSQL : localhost:5432"
	@echo "  OpenObserve: http://localhost:5080"

infra-down:
	docker compose -f docker-compose.yml down

infra-restart:
	docker compose -f docker-compose.yml restart

infra-logs:
	docker compose -f docker-compose.yml logs -f

infra-status:
	docker compose -f docker-compose.yml ps

# Internal — wait for PostgreSQL to accept connections
_wait-for-postgres:
	@echo "Waiting for PostgreSQL..."
	@for i in $$(seq 1 30); do \
		docker compose -f docker-compose.yml exec -T postgres \
			pg_isready -U $${POSTGRES_USER:-trader} >/dev/null 2>&1 && \
			echo "$(GREEN)PostgreSQL is ready.$(RESET)" && exit 0; \
		echo "  attempt $$i/30..."; \
		sleep 2; \
	done; \
	echo "ERROR: PostgreSQL did not become ready in time" && exit 1

# ─────────────────────────────────────────────────────────────────────────────
# Frontend
# ─────────────────────────────────────────────────────────────────────────────

frontend-install:
	cd $(FRONTEND_DIR) && npm install

frontend-build:
	cd $(FRONTEND_DIR) && npm run build

frontend-dev:
	cd $(FRONTEND_DIR) && npm run dev

frontend-lint:
	cd $(FRONTEND_DIR) && npx eslint . --max-warnings 0

frontend-fmt:
	cd $(FRONTEND_DIR) && npx prettier --write .

frontend-fmt-check:
	cd $(FRONTEND_DIR) && npx prettier --check .

# ─────────────────────────────────────────────────────────────────────────────
# Docker
# ─────────────────────────────────────────────────────────────────────────────

docker-build:
	@echo "$(BOLD)Building Docker image $(DOCKER_IMAGE):$(IMAGE_TAG)...$(RESET)"
	docker build \
		--tag $(DOCKER_IMAGE):$(IMAGE_TAG) \
		--tag $(DOCKER_IMAGE):latest \
		--file Dockerfile \
		.
	@echo "$(GREEN)Image built: $(DOCKER_IMAGE):$(IMAGE_TAG)$(RESET)"

docker-push: docker-build
	@echo "$(BOLD)Pushing to GHCR...$(RESET)"
	docker push $(DOCKER_IMAGE):$(IMAGE_TAG)
	docker push $(DOCKER_IMAGE):latest

docker-run:
	docker run --rm \
		--name $(APP_NAME) \
		--env-file .env.local \
		--network host \
		$(DOCKER_IMAGE):latest

docker-stop:
	docker stop $(APP_NAME) 2>/dev/null || true

# ─────────────────────────────────────────────────────────────────────────────
# Deploy (Hetzner Singapore)
# ─────────────────────────────────────────────────────────────────────────────

deploy: docker-push
	@echo "$(BOLD)Deploying $(IMAGE_TAG) to $(SSH_HOST)...$(RESET)"
	@test -n "$(SSH_HOST)" || (echo "ERROR: SSH_HOST not set. Add HETZNER_HOST to .env.local" && exit 1)
	ssh $(SSH_USER)@$(SSH_HOST) '\
		cd $(DEPLOY_DIR) && \
		docker compose -f docker-compose.prod.yml pull && \
		docker compose -f docker-compose.prod.yml up -d && \
		docker system prune -f \
	'
	@echo "$(GREEN)Deployed $(IMAGE_TAG) to $(SSH_HOST).$(RESET)"

deploy-migrate:
	@echo "Running migrations on prod..."
	ssh $(SSH_USER)@$(SSH_HOST) '\
		cd $(DEPLOY_DIR) && \
		docker compose -f docker-compose.prod.yml run --rm bot \
			sqlx migrate run --source /app/migrations \
	'

logs:
	ssh $(SSH_USER)@$(SSH_HOST) \
		"cd $(DEPLOY_DIR) && docker compose -f docker-compose.prod.yml logs -f bot"

ssh:
	ssh $(SSH_USER)@$(SSH_HOST)

server-status:
	ssh $(SSH_USER)@$(SSH_HOST) \
		"cd $(DEPLOY_DIR) && docker compose -f docker-compose.prod.yml ps"

# ─────────────────────────────────────────────────────────────────────────────
# Misc
# ─────────────────────────────────────────────────────────────────────────────

clean:
	cargo clean
	@if [ -d $(FRONTEND_DIR)/dist ]; then rm -rf $(FRONTEND_DIR)/dist; fi
	@echo "$(GREEN)Cleaned build artifacts.$(RESET)"
