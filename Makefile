WASM := target/wasm32-unknown-unknown/release/phylo_fasttree_wasm.wasm

.PHONY: build test browser-test browser-demo web clean

build:
	cargo build --target wasm32-unknown-unknown --release

# Regenerate JS glue for Node and run byte-for-byte comparison with native FastTree.
test: build
	wasm-bindgen $(WASM) --target nodejs --out-dir tests/pkg
	node tests/node_test.js

# Regenerate JS glue for browsers and run the headless-browser smoke test.
browser-test: build
	wasm-bindgen $(WASM) --target web --out-dir tests/pkg_web
	(cd tests && python3 -m http.server 8765 >/dev/null 2>&1 & echo $$! > /tmp/ft_http.pid; \
	  sleep 1; \
	  out=$$(chromium --headless --no-sandbox --disable-gpu --virtual-time-budget=10000 \
	    --dump-dom http://localhost:8765/browser_test.html 2>/dev/null); \
	  echo "$$out" | grep -q '2.1.11' && echo "$$out" | grep -q 'Cannot use -close'; \
	  status=$$?; \
	  kill $$(cat /tmp/ft_http.pid) 2>/dev/null || true; \
	  exit $$status)

# Regenerate JS glue for browsers (serve tests/ over HTTP and open browser.html).
browser-demo: build
	wasm-bindgen $(WASM) --target web --out-dir tests/pkg_web

# Build the upload/tree web interface (serve web/ over HTTP).
web: build
	wasm-bindgen $(WASM) --target web --out-dir web/pkg_web

clean:
	cargo clean
	rm -rf tests/pkg tests/pkg_web web/pkg_web
