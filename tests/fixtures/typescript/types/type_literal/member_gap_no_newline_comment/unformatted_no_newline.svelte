<script lang="ts">
	// A block comment sharing its line with the next member's start leads it
	// (the // comment forces the literal multiline).
	type T1 = {
		// x
		a: A; /* c1 */ b: B;
	};

	// Same in the width-broken layout.
	type T2 = { aaaaaaaaaaaaaaaaaaaaaa: AAAAAAAAAAAAAAAAAAAAAAAA; /* c2 */ bbbbbbbbbbbbbbbbbbbb: BBBBBBBBBBBBBBBBBB };

	// A comment before the member's separator trails it however the lines fall.
	type T3 = {
		// x
		c: C /* c3 */; d: D;
	};

	// A glued pair whose tail is a multi-line block still leads.
	type T4 = {
		// x
		e: E; /* c4 */ /* y
		z */ f: F;
	};
</script>
