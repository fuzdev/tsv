<script lang="ts">
	// Multi-arg call whose last arg is an arrow with a call body: the expand-last
	// break-body state (head arg stays inline, arrow body breaks after =>).
	const a = subscribe(initialState, (value) =>
		transform(value, contextValueOne, contextValueTwo, contextValueThree, contextFour)
	);

	// Multi-arg arrow with a CONDITIONAL (ternary) body: parens inline, breaks after =>.
	const b = subscribe(initialState, (value) =>
		value.ready ? transform(value, contextValueOne, contextValueTwo, contextThree) : idleState
	);

	// Nested multi-arg arrow-call bodies stay inline when they fit.
	const c = p.then(handler, (x) => q.then(handler, (y) => finalize(y)));
</script>
