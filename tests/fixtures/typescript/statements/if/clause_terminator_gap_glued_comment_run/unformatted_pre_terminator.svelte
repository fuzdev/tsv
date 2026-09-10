<script>
	// a glued block-comment pair in a clause tail takes one line per comment, the answer
	// the same gap gives once the `;` sits ahead of the run
	if (cond) expr1
	/* c1 */ /* c2 */
	;
	else fn1();
	// three of them, in authored order
	if (a) expr2
	/* c3 */ /* c4 */ /* c5 */
	;
	else fn2();
	// a `//` behind a glued block closes the line either way
	if (b) expr3
	/* c6 */ // c7
	;
	else fn3();
	// a multi-line block pair
	if (c) expr4
	/* c8
	d1 */ /* c9
	d2 */
	;
	else fn4();
	// an else-if chain's own clause body
	if (d) expr5;
	else if (e) expr6
	/* c10 */ /* c11 */
	;
	else fn5();
	// a glued pair ahead of an own-line third, and the mirror
	if (g) expr7
	/* c12 */ /* c13 */
	/* c14 */
	;
	else fn6();
	if (h) expr8
	/* c15 */
	/* c16 */ /* c17 */
	;
	else fn7();
	// CONTROL: a run the author glued to the CONTENT's own line is a TRAILING run and
	// keeps that line in both formatters — the gap's rule reaches only what starts below
	if (i) expr9 /* t1 */ /* t2 */
	;
	else fn8();
	// CONTROL: the two arms in one statement, a trailing comment ahead of an own-line pair
	if (j) expr10 /* t3 */
	/* c18 */ /* c19 */
	;
	else fn9();
	// CONTROL: the author's own-line run already takes one line per comment
	if (k) expr11
	/* c20 */
	/* c21 */
	;
	else fn10();
</script>
