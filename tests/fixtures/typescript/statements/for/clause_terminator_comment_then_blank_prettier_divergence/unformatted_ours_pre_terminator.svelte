<script>
	// the hoisted comment stays with its statement; the author blank follows it
	for (;;) continue
	// c1
	;

	fn1();
	// the block spelling of the same gap
	for (;;) break
	/* c2 */
	;

	fn2();
	// the rule is the SEAM's, not the clause's: a plain statement's own terminator gap
	// answers identically
	fn3()
	// c3
	;

	fn4();
	fn5()
	/* c4 */
	;

	fn6();
</script>
