<script lang="ts">
	// A block-body first callback + an empty cast-wrapped second arg with a leading
	// block comment. couldExpandArg looks through the cast to the (empty, so
	// non-expandable) collection, so expand-first fires and the comment stays inline
	// after `}` — the callback hugs, the comment leads the tail arg. Covers the plain
	// call, `new`, and member-chain expand-first paths.

	// plain call
	foo(() => {
		doThing();
	}, /* c */ {} as T);

	// new expression
	new Foo(() => {
		doThing();
	}, /* c */ {} as T);

	// member-chain call
	o.m(() => {
		doThing();
	}, /* c */ {} as T);
</script>
