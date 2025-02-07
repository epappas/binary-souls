PACKAGE_NAME := $(shell sed -En 's/name[[:space:]]*=[[:space:]]*"([^"]+)"/\1/p' Cargo.toml | head -1)
PACKAGE_VERSION := $(shell sed -En 's/version[[:space:]]*=[[:space:]]*"([^"]+)"/\1/p' Cargo.toml | head -1)
DOCKERHUB_ORG := binarysouls

CARGO := cargo
RUSTC := rustc

DEBUG_FLAGS := --features debug
RELEASE_FLAGS := --release

.DEFAULT_GOAL := help

help:
	@echo "Available targets:"
	@echo "  build              - Build the project in debug mode"
	@echo "  build-release      - Build the project in release mode"
	@echo "  run                - Run the project in debug mode"
	@echo "  run-release        - Run the project in release mode"
	@echo "  test               - Run all tests"
	@echo "  run-test TEST=name - Run a specific test"
	@echo "  debug TEST=name    - Debug a specific test"
	@echo "  bench              - Run benchmarks"
	@echo "  lint               - Run the linter (clippy)"
	@echo "  format             - Format the code with rustfmt"
	@echo "  check              - Check the code without building"
	@echo "  doc                - Generate documentation"
	@echo "  graph              - Generate dependency graph"
	@echo "  clean              - Clean up build artifacts"
	@echo "  update             - Update dependencies"
	@echo "  docker-build       - Build Docker image"
	@echo "  docker-push        - Push Docker image to registry"

build:
	$(CARGO) build

build-release:
	$(CARGO) build $(RELEASE_FLAGS)

run:
	$(CARGO) run

run-release:
	$(CARGO) run $(RELEASE_FLAGS)

test:
	$(CARGO) test --workspace

run-test:
	$(CARGO) test --test $(TEST)

debug:
	$(CARGO) test --test $(TEST) $(DEBUG_FLAGS)

bench:
	$(CARGO) bench

lint:
	$(CARGO) clippy -- -D warnings

format:
	$(CARGO) fmt

check:
	$(CARGO) check

doc:
	$(CARGO) doc --no-deps

graph:
	rm -f cargo-graph.dot
	rm -f cargo-graph.png
	$(CARGO) graph --optional-line-style dashed --optional-line-color red --optional-shape box --build-shape diamond --build-color green --build-line-color orange > cargo-graph.dot
	dot -Tpng > cargo-graph.png cargo-graph.dot

clean:
	$(CARGO) clean
	find . -type f -name "*.orig" -exec rm {} \;
	find . -type f -name "*.bk" -exec rm {} \;
	find . -type f -name ".*~" -exec rm {} \;

update:
	$(CARGO) update

docker-build:
	docker build -t $(PACKAGE_NAME):$(PACKAGE_VERSION) -f ./Dockerfile .

docker-push:
	docker tag $(PACKAGE_NAME):$(PACKAGE_VERSION) $(DOCKERHUB_ORG)/$(PACKAGE_NAME):$(PACKAGE_VERSION)
	docker push $(DOCKERHUB_ORG)/$(PACKAGE_NAME):$(PACKAGE_VERSION)

.PHONY: run-test debug run-tests bench lint graph clean docker-build docker-push
