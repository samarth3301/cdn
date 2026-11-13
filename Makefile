# Makefile for CDN TechSolace

# Variables
BINARY_NAME=cdn-techsolace
CARGO=cargo
DOCKER_COMPOSE=docker-compose -f docker/compose.yaml

.PHONY: all help build release run dev test clean fmt lint check setup docker-up docker-down docker-build

all: help

help: ## Display this help screen
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-20s\033[0m %s\n", $$1, $$2}'

setup: ## Setup initial project environment (copy .env.example)
	@if [ ! -f .env ]; then cp .env.example .env && echo ".env file created from .env.example"; else echo ".env already exists"; fi

build: ## Build the project in debug mode
	$(CARGO) build

release: ## Build the project in release mode
	$(CARGO) build --release

run: ## Run the project in debug mode
	$(CARGO) run

dev: setup ## Run the project with auto-reload (requires cargo-watch)
	@if command -v cargo-watch >/dev/null 2>&1; then \
		$(CARGO) watch -x run; \
	else \
		echo "cargo-watch not found, running with 'cargo run' instead..."; \
		$(CARGO) run; \
	fi

test: ## Run tests
	$(CARGO) test

clean: ## Clean the build directory
	$(CARGO) clean

fmt: ## Format the code
	$(CARGO) fmt

lint: ## Run clippy for linting
	$(CARGO) clippy -- -D warnings

check: ## Run cargo check
	$(CARGO) check

# Docker Targets
docker-build: ## Build docker images
	$(DOCKER_COMPOSE) build

docker-up: setup ## Start the project in docker
	$(DOCKER_COMPOSE) up -d

docker-down: ## Stop docker containers
	$(DOCKER_COMPOSE) down

docker-logs: ## View docker logs
	$(DOCKER_COMPOSE) logs -f

# Production Deployment (Alias for docker-up)
prod: docker-up ## Run the project in production mode (Docker)
