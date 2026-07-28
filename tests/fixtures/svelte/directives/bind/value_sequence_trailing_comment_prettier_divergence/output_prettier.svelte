<!-- a comment after the LAST operand of a function-binding sequence is preserved, like
	every other trailing position in a `{…}` value -->
<input bind:value={() => a, (v) => (a = v)} />

<!-- a trailing line comment ends the line, so the value breaks and the `}` reuses the break -->
<input bind:value={() => a, (v) => (a = v)} />

<!-- a comment before the comma trails the getter, where the author wrote it -->
<input bind:value={() => a /* c */, (v) => (a = v)} />
