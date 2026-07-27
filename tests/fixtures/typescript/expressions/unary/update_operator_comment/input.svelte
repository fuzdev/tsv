<script>
	// a line comment between a prefix operator and its operand stays glued to the operator
	++// c1
	x;

	// an own-line block comment does too
	++/* c2 */
	y;

	// a block comment glued to the operator leads the operand inline
	++/* c3 */ z;

	// on the postfix side the comment trails the operand and glues to the operator
	a /* c4 */++;

	b /* c5 */--;

	arr[0] /* c6 */++;
</script>
