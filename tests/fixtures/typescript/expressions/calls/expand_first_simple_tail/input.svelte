<script lang="ts">
	// A block-body first callback keeps a "simple" tail arg inline after `}`
	// (isSimpleCallArgument). A template literal, an update, and a unary with one of the
	// four operators that predicate accepts (`!`, `-`, `+`, `~`) all qualify, so each stays
	// inline; `typeof` / `void` and a meta property do not, so those break all args.

	// template literal tail
	foo(() => {
		doThing();
	}, `a${x}b`);

	// unary tail
	foo(() => {
		doThing();
	}, -x);

	// update tail
	foo(() => {
		doThing();
	}, x++);

	// `typeof` is not one of the four simple unary operators — all args break
	foo(
		() => {
			doThing();
		},
		typeof x
	);

	// nor is `void`
	foo(
		() => {
			doThing();
		},
		void x
	);

	// a meta property is not a simple argument — all args break
	foo(
		() => {
			doThing();
		},
		import.meta
	);
</script>
