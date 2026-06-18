.PHONY: demo test check-circuit prove build-witness clean-witness

demo:
	cargo run -- demo

test:
	cargo run -- test-all

check-circuit:
	cargo run -- check-circuit

prove:
	cargo run -- prove

build-witness:
	cargo run -- build-witness

clean-witness:
	rm -f Prover.toml Prover.regtest.toml target/zk_proof_of_hodl.gz
