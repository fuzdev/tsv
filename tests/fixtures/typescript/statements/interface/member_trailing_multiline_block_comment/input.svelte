<script lang="ts">
	// Trailing multiline block on the last member (kept trailing, not dropped)
	interface Last {
		a: number; /*
		 * trailing multiline
		 */
	}

	// Trailing multiline block on a non-last member (kept trailing, not relocated)
	interface Mid {
		a: number; /*
		 * trailing multiline
		 */
		b: string;
	}

	// A block comment on the multiline block's closing line trails the member too
	interface ClosingBlock {
		a: number; /*
		 * trailing multiline
		 */ /* c1 */
		b: string;
	}

	// A line comment on that closing line trails the member as well
	interface ClosingLine {
		a: number; /*
		 * trailing multiline
		 */ // c2
		b: string;
	}

	// The same run on the last member stays trailing before the closing brace
	interface ClosingLast {
		a: number; /*
		 * trailing multiline
		 */ /* c3 */
	}

	// Own-line multiline block leads the next member (unchanged)
	interface Leading {
		a: number;
		/*
		 * leading multiline
		 */
		b: string;
	}

	// Single-line block trailing a member (unchanged, already correct)
	interface Single {
		a: number; /* trailing */
	}
</script>
