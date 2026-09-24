<script>
	// a same-line comment left behind by a rest element's stripped parens rides the
	// target's line, in an array and an object assignment pattern alike
	[...a /* c */] = x;
	[b, ...a /* c */] = x;
	({ ...a /* c */ } = x);
	({ b, ...a /* c */ } = x);

	// a member target, a nested shell, and two comments across the layers
	[...a.b /* c */] = x;
	[...a[0] /* c */] = x;
	[...a /* c */] = x;
	[...a /* c1 */ /* c2 */] = x;

	// a line comment inside the stripped parens runs to end of line, so the pattern breaks
	[
		...a // c
	] = x;
	({
		...a // c
	} = x);
	[
		...a /* c1 */ // c2
	] = x;

	// an own-line comment is the pattern's share: a line of its own before the closer
	[
		...a
		// c
	] = x;
	[
		...a
		/* c */
	] = x;
	({
		...a
		// c
	} = x);
	({
		...a
		/* c */
	} = x);
	({
		...a /* c1 */
		/* c2 */
	} = x);
	[
		...a
		/* c1 */
		// c2
	] = x;

	// a comment after the `)` follows the interior's share; a line comment in the shell
	// still ends the line, so the block after the `)` lands ahead of it
	[...a /* c1 */ /* c2 */] = x;
	({ ...a /* c1 */ /* c2 */ } = x);
	[
		...a /* c2 */ // c1
	] = x;
	({
		...a /* c2 */ // c1
	} = x);

	// an own-line comment in the shell, then a line comment after the `)`: the pair keeps
	// the order the author wrote it in, the line comment trailing the one before it
	[
		...a
		/* c1 */ // c2
	] = x;
	({
		...a
		/* c1 */ // c2
	} = x);

	// an own-line comment in the shell, then a block after the `)`: the block hoists onto
	// the target's line, ahead of the one the shell held
	[
		...a /* c2 */
		/* c1 */
	] = x;
	({
		...a /* c2 */
		/* c1 */
	} = x);

	// a multi-line block comment keeps its lines and breaks the pattern
	[
		...a /* c1
	c2 */
	] = x;

	// an own-line multi-line block in the shell keeps its own line, in the object pattern
	// as in the array pattern, a block after the `)` hoisting ahead of it
	[
		...a
		/* c1
	c2 */
	] = x;
	({
		...a
		/* c1
	c2 */
	} = x);
	({
		b,
		...a
		/* c1
	c2 */
	} = x);
	({
		...a /* t */
		/* c1
	c2 */
	} = x);

	// a nested pattern, and a for-of head
	[b, [...a /* c */]] = x;
	({
		b: [...a /* c */]
	} = x);
	for ([...a /* c */] of xs);
	for ({
		...a // c
	} of xs);
</script>
