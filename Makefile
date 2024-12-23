.PHONY: version-map schemas build-contract build-category build-all unit-test integration-test test

# Generates a mapping between each contract and its version
version-map:
	@echo "Building version map..."
	@./scripts/build_version_map.sh
	@echo "Version map built! \033[0;32m\xE2\x9C\x94\033[0m"
# Generates the schema for each contract
schemas:
	@echo "Building schemas..."
	@./scripts/build_schema.sh
	@echo "Schemas built! \033[0;32m\xE2\x9C\x94\033[0m"
# Builds a single contract
build-contract:
	@echo "Building contract..."
	@read -p "Enter contract name: " contract_name && \
	./scripts/build.sh "$$contract_name"
	@echo "Build complete! \033[0;32m\xE2\x9C\x94\033[0m"

# Builds a category of contracts
build-category:
	@echo "Building category..."
	@read -p "Enter category name: " category_name && \
	./scripts/build.sh "$$category_name"
	@echo "Build complete! \033[0;32m\xE2\x9C\x94\033[0m"

# Builds all contracts and generates a version map
build:
	@echo "Building all contracts..."
	@./scripts/build_all.sh || exit 1
	@echo "Build complete! \033[0;32m\xE2\x9C\x94\033[0m"

# Attaches contract versions to the wasm files
attach-contract-versions:
	@echo "Attaching contract versions..."
	@./scripts/attach_contract_versions.sh
	@echo "Contract versions attached! \033[0;32m\xE2\x9C\x94\033[0m"

# Runs unit tests
unit-test:
	@echo "Running unit tests..."
	@cargo unit-test --workspace --quiet
	@echo "Unit tests complete! \033[0;32m\xE2\x9C\x94\033[0m"

# Runs integration tests
integration-test:
	@echo "Running integration tests..."
	@cargo test -p tests-integration --quiet
	@echo "Integration tests complete! \033[0;32m\xE2\x9C\x94\033[0m"

# Runs all tests
test: unit-test integration-test
	@echo "All tests complete! \033[0;32m\xE2\x9C\x94\033[0m"

# Deploys OS to specified blockchain
# Required env vars:
#   DEPLOYMENT_CHAIN - Chain ID or name (e.g., galileo-4)
#   TEST_MNEMONIC - Wallet mnemonic for deployment
# Optional env vars:
#   DEPLOYMENT_KERNEL_ADDRESS - For updating kernel address
#   SLACK_WEBHOOK_URL - For Slack notifications
deploy: build version-map
	@echo "Deploying OS..."
	@test -n "$$DEPLOYMENT_CHAIN" || (echo "Error: DEPLOYMENT_CHAIN is required" && exit 1)
	@test -n "$$TEST_MNEMONIC" || (echo "Error: TEST_MNEMONIC is required" && exit 1)
	@RUST_LOG=info cargo run --package andromeda-deploy
	@echo "OS deployed! \033[0;32m\xE2\x9C\x94\033[0m"
