<script>
	// A `@__PURE__` annotation binds to the token after it, so it is emitted by the node
	// that token begins. The call-argument and curried-arrow paths assemble an arrow from
	// its signature and body rather than building it as a whole expression, so they must
	// emit the annotation too — otherwise it is dropped.

	// sole arrow argument
	const a = map(/* @__PURE__ */ (n) => f(n));

	// arrow argument followed by another argument
	const b = map(/* @__PURE__ */ (n) => f(n), x);

	// `new` callee argument
	const c = new F(/* @__PURE__ */ (n) => f(n));

	// inner arrow of a curried chain
	const d = (n) => /* @__PURE__ */ (m) => f(n, m);

	// the call breaks, so the arrow is rebuilt from its signature and body
	const e = obj
		.keys(source)
		.map(/* @__PURE__ */ (n) => f(n, aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa));

	// an annotation leading the last argument refuses the expand-last hug, exactly as any
	// other leading comment does — the arguments break out instead (plain call and chain)
	f(
		'm',
		/* @__PURE__ */ (d) => {
			g(d);
		}
	);
	obj.on(
		'm',
		/* @__PURE__ */ (d) => {
			g(d);
		}
	);
</script>
