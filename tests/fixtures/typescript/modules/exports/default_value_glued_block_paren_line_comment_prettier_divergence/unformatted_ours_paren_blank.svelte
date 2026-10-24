<script lang="ts">
	// a block comment glued to a grouping `(` binds to the parenthesized value, so the pair is kept
	// when its interior opens with a `//`: stripped, the `//` would land between the two
	export default /* @__PURE__ */ (

		// side-effect free
		fn()
	);
	export = /* c2 */ (

		// d2
		y
	);
</script>
