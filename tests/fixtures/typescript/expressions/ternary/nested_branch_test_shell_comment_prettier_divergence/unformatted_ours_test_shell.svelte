<script>
	// A block comment inside the redundant paren shell around a NESTED conditional's test sits
	// in the enclosing branch gap once the shell is stripped, and takes that gap's rules in ONE
	// pass: a glued multi-line block leads the branch at its indent...
	const y1 = a ? b : (/* c1
	*/ aaa) ? bbb : ccc;

	// ...and an own-line single-line block collapses with the branch
	const y2 = a ? b : (
	/* c2 */
	aaa) ? bbb : ccc;

	// The consequent's test shell, same rule
	const y3 = a ? (/* c3
	*/ aaa) ? bbb : ccc : c;

	// A block after the nested conditional, inside the shell, defers past the statement's `;`
	const y4 = a ? b : (aaa ? bbb : ccc /* t */);
</script>
