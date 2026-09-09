<script lang="ts">
	// A line comment between the last operand and the sequence's closing paren has no
	// place inside the parens: it runs to end-of-line, so it renders after whatever
	// the enclosing construct prints on that line. The operand run stays on one line.
	const x1 = (a, b // c1
	) as A;
	const x2 = (a, b // c2
	) satisfies A;
	(a, b // c3
	) as A;
	const x3 = (a, b); // c4

	// A construct that does break puts the comment on its own last line.
	fn((a, b // c5
	) as A);

	// A comment in an operand GAP breaks the run — the comma is where it flushes.
	const x4 =
		(a, // c6
		b);

	// A hanging value position closes on its own line, so the comment lands inside.
	function fn1() {
		return (
			a,
			b // c7
		);
	}
</script>
