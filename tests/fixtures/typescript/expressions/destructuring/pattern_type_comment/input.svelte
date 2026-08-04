<script lang="ts">
	// A comment in a destructuring pattern's type annotation belongs to the type,
	// not to the last element before the `]` / `}`.
	const [a]: /* c1 */ T = x;
	const { b }: /* c2 */ U = y;
	const [c, d]: /* c3 */ V = z;
	function fn([e]: /* c4 */ W) {}

	// An element's own trailing comment stays distinct from the annotation's.
	const [f /* c5 */]: /* c6 */ X = x;
</script>
