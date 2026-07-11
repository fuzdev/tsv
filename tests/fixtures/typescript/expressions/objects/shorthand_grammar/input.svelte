<script lang="ts">
	// shorthand requires a plain identifier reference
	const valid = { a, b };
	// contextual keywords are valid identifier references, so valid as shorthand
	const contextual = { async, string };
	// the rejected shorthand forms are all valid WITH a value
	const withValue = { '': 1, 0: 2, [e]: 3, class: 4 };
	// destructuring keeps the identifier-reference initializer form (computed needs a value)
	({ a = 1, [e]: c = 2 } = obj);
</script>
