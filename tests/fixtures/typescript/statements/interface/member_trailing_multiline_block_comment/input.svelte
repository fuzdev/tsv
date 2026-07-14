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
