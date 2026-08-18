<script>
	// own-line tail run, then a block in the `;`→`else` gap: source order, block on its own line
	if (a) expr1;
	// c1
	/* c2 */
	else fn1();
	// a same-line trailer in the tail, then the gap block: the consequent stays collapsed
	if (b)
		expr2; // c3
	/* c4 */
	else fn2();
	// a lone gap block with no deferred tail stays on the `;` line
	if (c) expr3; /* c5 */
	else fn3();
	// an own-line block tail: the pair splits, in source order
	if (d) expr4;
	/* c6 */
	/* c7 */
	else fn4();
	// two gap blocks split too — the gap's own one-comment-per-line policy
	if (e) expr5;
	// c8
	/* c9 */
	/* c10 */
	else fn5();
</script>
