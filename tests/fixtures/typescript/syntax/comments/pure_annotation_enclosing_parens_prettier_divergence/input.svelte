<script>
	// A `@__PURE__` annotation marks the call that FOLLOWS it as side-effect-free, so a
	// bundler may drop it when its result is unused. An enclosing paren the formatter
	// adds around an expression whose left edge is an annotated call goes OUTSIDE the
	// comment — the annotation must stay glued to its call, or it leads a paren instead
	// and the call is no longer treated as pure.

	// clarity parens around a `??` ternary operand
	const a = cond ? p : (/* @__PURE__ */ f() ?? y);
	const b = cond ? (/* @__PURE__ */ f() ?? y) : p;
	const c = (/* @__PURE__ */ f() ?? y) ? p : q;

	// required parens: `??` mixed with `||`, member base, callee, `new` callee, `**` operand
	const d = (/* @__PURE__ */ f() ?? y) || p;
	const e = (/* @__PURE__ */ f() ?? y).n;
	const g = (/* @__PURE__ */ f() ?? y)();
	const h = new (/* @__PURE__ */ f() ?? y)();
	const i = (/* @__PURE__ */ f() ?? y) ** 2;

	// a sequence keeps its value-position parens
	const j = (/* @__PURE__ */ f(), y);

	// a unary operand takes the wrapping pair: bare, the annotation would read as
	// marking the operator rather than the call (both formatters wrap)
	const n = !(/* @__PURE__ */ f());
	const o = -(/* @__PURE__ */ f());

	// rollup and terser spell it `#__PURE__`
	const k = cond ? p : (/*#__PURE__*/ f() ?? y);

	// an annotated `new` follows the same rule
	const l = cond ? p : (/* @__PURE__ */ new F() ?? y);

	// no enclosing paren: the annotation already sits against its call
	const m = /* @__PURE__ */ f() ?? y;
</script>
