<script>
	// a glued block pair in the `do` body's terminator gap takes one line per comment
	do expr1;
	/* c1 */
	/* c2 */
	while (a);
	// a `//` behind a glued block closes the line either way
	do expr2;
	/* c3 */
	// c4
	while (b);
	// an authored blank between two remarks survives the split, one comment per line
	do expr3;
	/* c5 */
	/* c6 */

	/* c7 */
	while (c);
	// CONTROL: a run glued to the body's own line is a TRAILING run and keeps that line
	do expr4; /* t1 */ /* t2 */
	while (d);
</script>
