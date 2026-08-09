<script>
	switch (x) {
		// A block comment between `case` and its test stays where the author wrote it —
		// including when the test's parens are the printer's to re-synthesize, so the
		// comment has no node of its own to ride inside.
		case /* c */ (a, b):
			break;
		case /* c */ (a = 1):
			break;
		case /* c */ a:
			break;
		case /* p */ /* q */ b:
			break;
		case /* multi
		line */ c:
			break;
	}
</script>
