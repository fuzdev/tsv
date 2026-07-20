<script>
	// A block-body first callback + a JSDoc-cast tail. The cast keeps its parens, so the
	// tail arg is a paren node carrying no comment of its own: the leading-comment gate
	// never sees the annotation, couldExpandArg can't reach the wrapped collection, and
	// expand-first fires. Contrast `expand_first_wrapped_second_arg`, where an `as` cast
	// is transparent to couldExpandArg and a non-empty collection breaks all args.

	// plain call, empty object tail
	foo(() => {
		doThing();
	}, /** @type {T} */ ({}));

	// new expression
	new Foo(() => {
		doThing();
	}, /** @type {T} */ ({}));

	// member-chain call
	o.m(() => {
		doThing();
	}, /** @type {T} */ ({}));

	// a non-empty object stays short — the paren hides it from couldExpandArg
	foo(() => {
		doThing();
	}, /** @type {T} */ ({ a: 1 }));

	// identifier tail
	foo(() => {
		doThing();
	}, /** @type {T} */ (a));

	// nested casts — both wrappers are transparent
	foo(() => {
		doThing();
	}, /** @type {A} */ (/** @type {B} */ (b)));

	// a plain block comment ahead of the cast doesn't block the hug
	foo(() => {
		doThing();
	}, /* c */ /** @type {T} */ ({}));

	// the wrapped expression is re-asked the shortness question, so the call-arity rule
	// still fires through the cast: one argument stays short ...
	foo(() => {
		doThing();
	}, /** @type {T} */ (fn(a)));

	// ... but two arguments are not short, and all args break
	foo(
		() => {
			doThing();
		},
		/** @type {T} */ (fn(a, b))
	);

	// a cast-wrapped conditional is NOT short — all args break
	foo(
		() => {
			doThing();
		},
		/** @type {T} */ (a ? b : c)
	);

	// a cast-wrapped arrow tail is NOT short — all args break
	foo(
		() => {
			doThing();
		},
		/** @type {T} */ (
			(b) => {
				doThing();
			}
		)
	);
</script>
