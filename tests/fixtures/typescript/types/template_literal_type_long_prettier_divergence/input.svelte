<script lang="ts">
	// After = break, template line is 100 chars - both stay inline
	type Short =
		`prefix_${SomeVeryLongTypeNameCccccccccccccccccccccccccccccccccccccccccccccccccccccccc}_suffix`;

	// After = break, template line is 101 chars - Prettier inline, we break at ${
	type Long =
		`prefix_${
			SomeVeryLongTypeNameCcccccccccccccccccccccccccccccccccccccccccccccccccccccccc
		}_suffix`;

	// Multiple interpolations - both break at =, we also break at ${
	type T =
		`${TypeA}_${TypeB}_${
			SomeVeryLongTypeNameCcccccccccccccccccccccccccccccccccccccccccccccccccccc
		}`;

	// Multiple long interpolations - each one that exceeds 100 chars breaks
	type U =
		`${TypeA}_${
			TypeBbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
		}_${
			SomeVeryLongTypeNameCccccccccccccccccccccccccccccccccccccccccccccccccccccc
		}`;
</script>
