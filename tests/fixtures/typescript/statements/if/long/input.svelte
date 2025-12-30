<script lang="ts">
	// With braces

	// 100 chars exactly - stays inline (at boundary)
	if (condA && condB && condC && condD && condE && condF && condG && condH && condI && condJJJJJJ) {
	}

	// 101 chars - wraps (exceeds print_width)
	if (
		condA &&
		condB &&
		condC &&
		condD &&
		condE &&
		condF &&
		condG &&
		condH &&
		condI &&
		condJJJJJJJ
	) {
	}

	// No braces, body on same line (entire statement fits)

	// 100 chars exactly - entire statement stays on one line
	if (condA && condB && condC && condD && condE && condF && condG && condH && condI && condJJJJJ) a;

	// No braces, body on next line (condition too long for body to fit)

	// 100 chars for condition only - body goes to next line
	if (condA && condB && condC && condD && condE && condF && condG && condH && condI && condJJJJJJJJ)
		a;

	// 101 chars - condition wraps
	if (
		condA &&
		condB &&
		condC &&
		condD &&
		condE &&
		condF &&
		condG &&
		condH &&
		condI &&
		condJJJJJJJJJ
	)
		a;
</script>
