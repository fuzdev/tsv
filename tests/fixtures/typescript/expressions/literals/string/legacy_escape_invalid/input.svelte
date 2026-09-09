<script>
	// A legacy octal escape (`\7`) and a non-octal decimal escape (`\8`) are disallowed
	// in strict code, and a module is always strict — the invalid siblings pin that.
	// What stays legal: `\0` with no decimal digit after it (the NUL escape), and a
	// digit after an escaped backslash, which opens no escape at all.
	const nul = '\0';
	const nulThenLetter = '\0x';
	const escapedBackslashDigit = '\\7';
</script>
