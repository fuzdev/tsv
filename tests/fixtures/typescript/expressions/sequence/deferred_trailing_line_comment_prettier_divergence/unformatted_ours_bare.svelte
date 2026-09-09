<script lang="ts">
	// A line comment between the last operand and the sequence's closing paren has no
	// place inside the parens: it runs to end-of-line, so it renders after whatever
	// the enclosing construct prints on that line. The operand run stays on one line.
	const x1 = (a, b) as A; // c1
	const x2 = (a, b) satisfies A; // c2
	(a, b) as A; // c3
	const x3 = (a, b // c4
	);

	// A construct that does break puts the comment on its own last line.
	fn(
		(a, b) as A // c5
	);

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
