<script lang="ts">
	// A template has no legacy escapes: `\0` before a digit, `\1`–`\7` and `\8` / `\9`
	// are a NotEscapeSequence, so a tagged template's cooked value is null.
	const octal = tag`\75 a`;
	const octalRun = tag`\101`;
	const zeroDigit = tag`\01`;
	const nonOctal = tag`\8`;
	const nonOctal9 = tag`\9`;
	const member = String.raw`\7`;

	// A bare `\0` is the NUL escape and still cooks.
	const nul = tag`\0`;
</script>
