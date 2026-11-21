WASM := target/wasm32-unknown-unknown/release/phylo_fasttree_wasm.wasm
# Double-precision build lives in its own target dir so the two don't evict each other.
WASM_DOUBLE := target/double/wasm32-unknown-unknown/release/phylo_fasttree_wasm.wasm

.PHONY: build build-double test test-double browser-test browser-demo web web-double clean

build:
	cargo build --target wasm32-unknown-unknown --release

build-double:
	cargo build --target wasm32-unknown-unknown --release --features double --target-dir target/double

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

# Double-precision browser package: web/pkg_web_double/phylo_fasttree_wasm_double.*
web-double: build-double
	wasm-bindgen $(WASM_DOUBLE) --target web --out-dir web/pkg_web_double --out-name phylo_fasttree_wasm_double

# Double-precision Node package + byte-for-byte check against a strict
# double-precision native FastTree (FT_DOUBLE=path; see README "Precision").
test-double: build-double
	wasm-bindgen $(WASM_DOUBLE) --target nodejs --out-dir tests/pkg_double --out-name phylo_fasttree_wasm
	node tests/double_test.js

clean:
	cargo clean
	rm -rf tests/pkg tests/pkg_web tests/pkg_double web/pkg_web web/pkg_web_double
