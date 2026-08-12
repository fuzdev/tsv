<script lang="ts">
	// a line comment sharing the `&`'s line keeps that line, so the trailing object
	// drops to the next one — the paren shell and the outer union expand with it
	type B1 =
		| (a & // c
				{ x: X })
		| c;

	// an own-line comment between the `&` and the trailing object keeps its own line
	type B2 =
		| (a &
				// c
				{ x: X })
		| c;
</script>
