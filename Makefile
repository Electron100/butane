CARGO := cargo +stable
CARGO_NIGHTLY := $(subst stable,nightly,$(CARGO))

all : build

build :
	$(CARGO) check
	# build some intermediate configuration to test different feature combinations
	cd butane && $(CARGO) check --features pg
	cd butane && $(CARGO) check --features pg,datetime
	cd butane && $(CARGO) check --features sqlite
	cd examples/getting_started && $(CARGO) check --features "sqlite,sqlite-bundled"
	cargo build --all-features

lint :
	$(CARGO) clippy --all-features -- -D warnings

lint-ci : doclint lint spellcheck check-fmt update-help-md check-help-md

check : build doclint lint spellcheck check-fmt test


test :
	$(CARGO) test --all-features
	# And run the example tests separately to avoid feature combinations
	cd examples; for dir in *; do cargo +stable test -p $$dir --all-features; done

clean :
	$(CARGO) clean


fmt :
	$(CARGO_NIGHTLY) fmt

check-fmt :
	$(CARGO_NIGHTLY) fmt --check
	editorconfig-checker

update-help-md :
	$(CARGO) run -p butane_cli --features clap-markdown -q -- --markdown-help list > HELP.md

check-help-md :
	git diff --exit-code HELP.md

spellcheck :
	typos

doclint :
	RUSTDOCFLAGS="-D warnings" RUSTFLAGS="-A elided_named_lifetimes" $(CARGO_NIGHTLY) doc --no-deps --all-features

doc :
	cd butane && $(CARGO_NIGHTLY) doc --no-deps --all-features

docview :
	cd butane && $(CARGO_NIGHTLY) doc --all-features --open

install :
	cd butane_cli && $(CARGO) install --path .

regenerate-example-migrations :
	for dir in examples/*; do \
		(cd $$dir; cargo +stable run -p butane_cli --all-features -- regenerate; \
		cargo +stable run -p butane_cli --all-features -- embed); \
	done
