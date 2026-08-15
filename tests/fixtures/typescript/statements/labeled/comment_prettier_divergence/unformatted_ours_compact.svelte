<script>
	label1:// comment after label
	for(;;){break;}

	// A run keeps the lines the author gave it, in the position tsv holds it
	label2:/* cmt1 */
	// cmt2
	fn();

	label3:/* cmt3 */
	/* cmt4 */
	fn();

	// A pair the author glued stays on one line, and a lone block collapses
	label4:/* cmt5 */ /* cmt6 */ fn();

	label5:/* cmt7 */ fn();
</script>
