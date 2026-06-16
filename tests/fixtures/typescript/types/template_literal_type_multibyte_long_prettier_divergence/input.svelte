<script lang="ts">
	// Multibyte quasi fits on one line - both formatters keep it inline
	type Inline = `名名名名名名名名名名名名名名名名名名名名名名名名名名名名名名名名名${T}`;

	// After = break, continuation is 100 visual cols - both stay inline
	type Fits =
		`名名名名名名名名名名名名名名名名名名名名名名名名名名名名名名名名名名名名名名名名名名名名a${T}`;

	// After = break, continuation is 101 visual cols - Prettier keeps inline, we break at ${
	type Over =
		`名名名名名名名名名名名名名名名名名名名名名名名名名名名名名名名名名名名名名名名名名名名名aa${
			T
		}`;
</script>
