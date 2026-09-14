<script lang="ts">
	function f(x): x /* b */ is A {} // c

	function g(x): x is B /* t */ {} // c

	// A run the author GLUED — a multi-line block ahead of a single-line one — and then broke
	// after. The run's break is forced by the multi-line block, so the predicate type hangs
	// below the whole run exactly as it does under the `//` above.
	function h(x): x /* a
b */ /* c */ is A {}
</script>
