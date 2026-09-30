<script lang="ts">
	// A comment between a statement keyword and its `(`, or after its `)`, leaves the `)`
	// a header's: the `<` after it opens an assertion, not a type-argument list, and the
	// regex after that holds a `)` the parameter list must not close at.
	const i = (
		x = () => {
			if (/* x */ c) /* y */ <RegExp>/[)]/;
		}
	) => x;
	function fn(
		a = () => {
			while (/* x */ c) /* y */ <RegExp>/[)]/.test(d);
		},
		b /* c */
	) {}
</script>
