<script>
	// comment after label
	label1: for (;;) {
		break;
	}

	// A run keeps the lines the author gave it, in the position tsv holds it
	/* cmt1 */
	// cmt2
	label2: fn();

	/* cmt3 */
	/* cmt4 */
	label3: fn();

	// A pair the author glued stays on one line, and a lone block collapses
	label4: /* cmt5 */ /* cmt6 */ fn();

	label5: /* cmt7 */ fn();
</script>
