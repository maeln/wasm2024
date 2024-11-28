calc:
    cd calc && just build && cp -f target/wasm32-unknown-unknown/debug/calc.wasm ../hcr/calc.wasm
test:
    curl 127.0.0.1:8000
