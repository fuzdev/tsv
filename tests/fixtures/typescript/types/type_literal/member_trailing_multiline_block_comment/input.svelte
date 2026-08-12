<script lang="ts">
	// Trailing multiline block on a non-last member (kept trailing, not relocated)
	type T1 = {
		a: A; /*
		 * trailing multiline
		 */
		b: B;
	};

	// A block comment on the multiline block's closing line trails the member too
	type T2 = {
		a: A; /*
		 * trailing multiline
		 */ /* c1 */
		b: B;
	};

	// A line comment on that closing line trails the member as well
	type T3 = {
		a: A; /*
		 * trailing multiline
		 */ // c2
		b: B;
	};

	// The same run on the last member stays trailing before the closing brace
	type T4 = {
		a: A; /*
		 * trailing multiline
		 */ /* c3 */
	};
</script>
