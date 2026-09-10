<script lang="ts">
	// Line comments: an authored blank between two own-line comments is preserved
	if (a) {
		f();
	}
	// c1

	// c2
	else {
		g();
	}

	// Block comments follow the same rule
	if (a) {
		f();
	}
	/* c3 */

	/* c4 */
	else {
		g();
	}

	// Every blank in a longer run is preserved
	if (a) {
		f();
	}
	// c5

	// c6

	// c7
	else {
		g();
	}

	// else-if is the same gap and the same rule
	if (a) {
		f();
	}
	// c8

	// c9
	else if (b) {
		g();
	}
	// A non-block consequent reaches this gap through its own terminator, and the blank
	// survives the hoist past the `;`
	if (a) f()
	// c10

	// c11
	;
	else g();

	// A glued run in that terminator gap breaks between its members and still keeps the
	// authored blank between the two remarks
	if (a) f()
	/* c12 */ /* c13 */

	/* c14 */ /* c15 */
	;
	else g();
</script>
