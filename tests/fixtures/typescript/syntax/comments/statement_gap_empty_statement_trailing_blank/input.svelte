<script>
	// A blank the author left before a dropped `;` is not a blank before the body's
	// trailing comment run — the cursor stands past the `;`, where nothing was written.
	function fn1() {
		a1();
		/* c1 */
	}

	// A blank written before the comment itself is still preserved.
	function fn2() {
		a3();

		/* c3 */
	}

	// The same rule at the top level, whose run closes against no `}`.
	a2();
	/* c2 */
</script>
