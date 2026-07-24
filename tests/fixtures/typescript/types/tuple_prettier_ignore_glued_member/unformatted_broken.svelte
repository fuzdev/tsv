<script lang="ts">
	// a block directive glued directly before a member freezes that member whole
	// (matches prettier); the inline tuple stays inline, separators parent-owned
	type A = [
		a,
		/* prettier-ignore */ {x:   1},
		b
	];

	// other comments in the glued run are transparent — the directive freezes the member
	// from anywhere in the run (matches prettier); only a separator breaks the glue
	type B = [a,   /* prettier-ignore */ /* other */ {x:   1},  b];
</script>
