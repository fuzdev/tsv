<script lang="ts">
	// Mixed: a block then a line comment leading the extends-type. The block trails
	// `extends` inline, the line comment forces the extends-type onto its own line.
	type T1 = A /* b */ extends B // c
		? X
		: Y;

	// Trailing: a line comment leading, a block trailing the extends-type. The line
	// hangs the type; the trailing block stays inline at this type position.
	type T2 = A extends B /* t */ // c
		? X
		: Y;
</script>
