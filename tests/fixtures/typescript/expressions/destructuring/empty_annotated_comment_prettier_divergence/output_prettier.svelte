<script lang="ts">
	// An empty pattern's interior comment stays inside the brackets the author
	// wrote it in, annotation or not.
	const {} /* c1 */ : T = x;
	const [] /* c2 */ : U = y;

	// Inside and outside are distinct positions, and both are kept.
	const {} /* c3 */ /* c4 */ : V = z;
</script>
