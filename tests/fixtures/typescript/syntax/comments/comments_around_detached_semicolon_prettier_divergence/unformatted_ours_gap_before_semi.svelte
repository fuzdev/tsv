<script lang="ts">
	// an own-line block in the terminator gap, and a block trailing the `;`
	fn1()
	/* c1 */
	; /* c2 */
	fn2();

	// the same with a `//` trailing the `;`
	fn3()
	/* c3 */
	; // c4
	fn4();

	// a run in the gap
	fn5()
	/* c5 */
	/* c6 */
	; /* c7 */
	fn6();

	// CONTROL: a gap comment on the CONTENT's line joins the run, in order, on both
	fn7() /* c8 */
	; /* c9 */
	fn8();

	// CONTROL: a `//` in the gap closes the line, so the trailer takes its own — on both
	fn9() // c10
	; /* c11 */
	fn10();
</script>
