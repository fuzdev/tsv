<script>
	// Declarator: the comment glued to `=>` ends its line, so it prints on its own line under
	// `=>` and the chain stays flat, the ternary body hugging
	const v1 = a.b().fnName((p) => /* c */
		aaaaaaaaaa ? bbbbbbbbbbbbbbbbbbbbbbbbbb : ccccccccccccccccccccccccccccccccccccccccccccc);

	// Assignment, a call body
	v2 = a.b().fnName((p) => /* c */
		gggggg(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb));

	// Return
	function q() {
		return a.b().fnName((p) => /* c */
			aaaaaaaaaa ? bbbbbbbbbbbbbbbbbbbbbbbbbb : ccccccccccccccccccccccccccccccccccccccccccccc);
	}

	// Property value, a binary body: the arguments break out
	const o = {
		k: a.b().fnName((p) => /* c */
			aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa + bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb)
	};

	// Call argument, an identifier body
	outer(
		a.b().fnName((p) => /* c */
			yyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy)
	);

	// A statement whose head does not merge
	abcdef.b().fnName((p) => /* c */
		aaaaaaaaaa ? bbbbbbbbbbbbbbbbbbbbbbbbbb : ccccccccccccccccccccccccccccccccccccccccccccc);

	// At 100 the arrow fits flat on the broken-out argument line, so the chain breaks and the
	// comment stays inline
	const v3 = a.b().fnName((p) => /* c */
		yyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy);

	// At 101 the arrow fits on no line: the chain stays flat and the comment prints on its own line
	const v4 = a.b().fnName((p) => /* c */
		yyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy);

	// At 100 a call body fits flat on the expanded chain's last line, so the chain breaks and the
	// comment stays inline
	const v6 = a.b().fnName((p) => /* c */
		gggggg(aaaa, bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb));

	// At 101 the call body fits on no line: the chain stays flat and the comment prints on its
	// own line
	const v7 = a.b().fnName((p) => /* c */
		gggggg(aaaa, bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb));

	// A sequence body breaks the arguments out
	const v9 = a.b().fnName((p) => /* c */
		(aaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb));

	// A blank line after the comment is kept
	const v10 = a.b().fnName((p) => /* c */

		gggggg(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb));

	// Control: a short-head statement merges its head, so the chain never reaches the two-state
	// group and the comment prints on its own line in both formatters
	a.b().fnName((p) => /* c */
		aaaaaaaaaa ? bbbbbbbbbbbbbbbbbbbbbbbbbb : ccccccccccccccccccccccccccccccccccccccccccccc);

	// Control: a two-comment run keeps the chain broken in both formatters
	const v5 = a.b().fnName((p) => /* c */ /* d */
		aaaaaaaaaa ? bbbbbbbbbbbbbbbbbbbbbbbbbb : ccccccccccccccccccccccccccccccccccccccccccccc);

	// Control: a second comment trailing the body keeps the chain broken in both formatters
	const v11 = a.b().fnName((p) => /* c */
		gggggg(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbbbbbbb) /* t */);

	// Control: a second comment ahead of the first argument keeps the chain broken in both
	// formatters
	const v12 = a.b().fnName(/* l */ (p) => /* c */
		gggggg(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbbbbbbb));

	// Control: an earlier call takes a function argument, so the chain stays broken in both
	// formatters
	const v8 = a.b(() => 1).fnName((p) => /* c */
		aaaaaaaaaa ? bbbbbbbbbbbbbbbbbbbbbbbbbb : ccccccccccccccccccccccccccccccccccccccccccccc);
</script>

<!-- Template expression -->
<p>
	{a.b().fnName((p) => /* c */
		aaaaaaaaaa ? bbbbbbbbbbbbbbbbbbbbbbbbbb : ccccccccccccccccccccccccccccccccccccccccccccc)}
</p>
