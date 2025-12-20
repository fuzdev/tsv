<style>
	div {
		/* Lone high surrogate (U+D800 - first in range) */
		content: '\D800';
		/* Lone low surrogate (U+DFFF - last in range) */
		content: '\DFFF';
		/* Surrogate pair sequence (two escapes, not combined) */
		content: '\D83D\DC69';
		/* Single codepoint form (proper way to write U+1F469) */
		content: '\1F469';
		/* Replacement character for reference */
		content: '\FFFD';
	}
</style>
