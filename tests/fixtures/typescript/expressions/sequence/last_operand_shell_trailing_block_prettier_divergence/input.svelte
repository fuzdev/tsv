<script lang="ts">
	// A block comment ending a redundant pair around a sequence's last operand trails the
	// pair the sequence prints around its operands.
	f((a, b) /* c1 */);

	f((a, b < c) /* c2 */);

	if ((a, b < c) /* c3 */) {
	}

	x = (a, b < c) /* c4 */ ? d : e;

	o = { k: (a, b < c) /* c5 */ };

	f((a, (b, c)) /* c6 */);

	// A multi-line block takes the same place.
	f(
		(a, b < c) /* c7
		 */
	);

	if (
		(a, b < c) /* c8
		 */
	) {
	}

	// A run of blocks on the operand's line, a multi-line one first.
	f(
		(a, b < c) /* c9
		 */ /* d */
	);

	// At a statement the comment trails the statement, past the `;`, with the author's pair
	// around the sequence or without it.
	(a, b < c); /* c10 */

	(a, b); /* c11 */

	(a, b < c); /* c12 */
</script>
