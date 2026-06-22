<script lang="ts">
	// Short function-type member stays inline (no boundary needed)
	type Short = (() => void) | x;

	// Function-type member, params fit at exactly 100 - stays inline (no divergence)
	type Fit =
		| ((aaaaaaaa: string, bbbbbbbb: string, cccccccc: string, dddddddddddddddddddd: string) => void)
		| xx;

	// Function-type member, params at 101 - break; closing `) => void)` diverges
	type Brk =
		| ((
				aaaaaaaa: string,
				bbbbbbbb: string,
				cccccccc: string,
				ddddddddddddddddddddd: string
			) => void)
		| xx;
</script>
