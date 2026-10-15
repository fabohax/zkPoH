.PHONY: demo test check-circuit build-witness prove verify gates update-trusted-vk clean-witness

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

# Checks target/vk against trusted_vk.sha256 and target/public_inputs against the
# snapshot's root and threshold, then runs bb verify.
verify:
	cargo run -- verify

# After an intentional circuit change: regenerate the vk with `make prove`, then pin it.
update-trusted-vk:
	sha256sum target/vk | cut -d' ' -f1 > trusted_vk.sha256
	@echo "trusted_vk.sha256 = $$(cat trusted_vk.sha256)"

gates:
	nargo compile && bb gates -b ./target/zkpoh.json

clean-witness:
	rm -f Prover.toml Prover.regtest.toml target/zkpoh.gz target/proof target/public_inputs target/vk target/vk_hash
