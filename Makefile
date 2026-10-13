.PHONY: demo test check-circuit build-witness prove verify gates clean-witness

NARGO_VERSION := 1.0.0-rc.4
BB_VERSION := 5.0.0

demo:
	cargo run -- demo

test:
	cargo run -- test-all

check-circuit:
	cargo run -- check-circuit

build-witness:
	cargo run -- build-witness

# Witness -> nargo compile -> nargo execute -> bb prove. Outputs in target/.
prove:
	cargo run -- prove

verify:
	cargo run -- verify

gates:
	nargo compile && bb gates -b ./target/zkpoh.json

clean-witness:
	rm -f Prover.toml Prover.regtest.toml target/zkpoh.gz target/proof target/public_inputs target/vk target/vk_hash
