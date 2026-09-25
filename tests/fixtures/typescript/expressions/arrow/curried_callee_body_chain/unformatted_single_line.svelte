<script>
	// A curried chain as a callee opens its parens onto their own lines when its own body
	// hangs, however a chain nested in it resolved

	// A short chain in the body stays inline, and the `)` still closes on its own line
	((a) => (b) => fn((c) => (d) => e))();
	((a) => (b) => x || ((c) => (d) => e))();
	new ((a) => (b) => fn((c) => (d) => e))();
	((a) => (b) => fn((c) => (d) => e))().p.q();
	((a) => (b) => fn((c) => (d) => e))?.();
	((a) => (b) => fn(function () { return (c) => (d) => e; }))();

	// A chain in a parameter default forces the heads to break, and a hugging body hugs the
	// last head inside the open parens
	((a = (x) => (y) => z) => (b) => ({ k: 1 }))();

	// The same chain with a plain body: the body hangs one level under the last head, and
	// the `)` closes on its own line
	((a = (x) => (y) => z) => (b) => c)();

	// Two chains nested in the body, in either order: one breaks, one stays inline, and
	// neither decides the enclosing chain's `)`
	((a) => (b) => fn([(x = 1) => (y) => z], (c) => (d) => e))();
	((a) => (b) => fn((c) => (d) => e, [(x = 1) => (y) => z]))();
</script>
