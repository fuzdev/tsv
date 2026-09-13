<style>
	div {
		/* a `:` with nothing after it — inside a group, and at the top level of a custom
		property's value — ends the value at the colon */
		a: f(a:);
		--b: a:;
		--c: :;
		--d: a:b:;
	}
</style>
