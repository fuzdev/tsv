<script lang="ts">
	// A multi-line block in a sealed base's leading gap leads the operand inside the
	// pair, glued through any stripped parens.
	(/* c1
	 */ x.s?.q).c;

	// A chain that breaks under it hugs the pair: the block keeps the `(` line and the
	// links take one indent.
	(/* c2
	 */ x.s
		?.m()
		.m()
		.c(() => d)).c;
</script>
