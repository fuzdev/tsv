<script lang="ts">
	// A block-body first callback + a TS-cast-wrapped tail. Prettier's couldExpandArg
	// recurses through `as`/`satisfies`/`<T>` to the wrapped collection, so a non-empty
	// object/array makes !couldExpandArg false and all args break — only a truly-empty
	// wrapped collection stays "short" and expands-first (inline after `}`).

	// `as`-wrapped non-empty object tail — all args break
	foo(
		() => {
			doThing();
		},
		{ a: 1 } as T
	);

	// `satisfies`-wrapped non-empty object tail — all args break
	foo(
		() => {
			doThing();
		},
		{ a: 1 } satisfies T
	);

	// angle-bracket-assertion non-empty object tail — all args break
	foo(
		() => {
			doThing();
		},
		<T>{ a: 1 }
	);

	// `as const`-wrapped non-empty array tail — all args break
	foo(
		() => {
			doThing();
		},
		[a, b] as const
	);

	// empty `as`-wrapped object tail is still short — expand-first keeps it inline
	foo(() => {
		doThing();
	}, {} as T);
</script>
