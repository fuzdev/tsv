<script lang="ts">
	// A block comment between the operand and the `)` keeps the position the
	// author gave it, and the operand's parens are retained for its sake.
	const a = <T>(x /* c */);
	const b = <T>(y /* c1 */ /* c2 */);

	// An operand whose parens are required takes the same single pair.
	const c = <T>(m + n /* c */);

	// The gap is empty here, so the redundant parens strip as usual.
	const d = <T>z;
</script>
