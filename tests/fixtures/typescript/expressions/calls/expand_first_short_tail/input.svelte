<script lang="ts">
	// A block-body first callback expands "first" and keeps a SHORT tail arg inline
	// after `}`. A call/new tail with more than one argument is not "hopefully short"
	// (isHopefullyShortCallArgument), so all args break instead.

	// tail is a call with 1 arg — short, so expand-first keeps it inline
	foo(() => {
		doThing();
	}, bar(x));

	// tail is a call with 2 args — not short, so all args break
	foo(
		() => {
			doThing();
		},
		bar(x, y)
	);

	// tail is a `new` with 1 arg — short, expand-first keeps it inline
	foo(() => {
		doThing();
	}, new Bar(x));

	// tail is a `new` with 2 args — not short, so all args break
	foo(
		() => {
			doThing();
		},
		new Bar(x, y)
	);
</script>
