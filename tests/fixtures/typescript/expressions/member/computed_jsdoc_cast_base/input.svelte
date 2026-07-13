<script>
	// A JSDoc cast base whose parens hang across lines: the optional computed lookup
	// still hugs the `)`. Prettier never breaks before a computed `[` (member.js
	// `shouldInline` includes `node.computed`); the brackets break instead.
	const a = /** @type {Record<string, Function | Function[]>} */ (
		componentContext.instance.$$events
	)?.[/** @type {string} */ (type)];

	// Same rule for a non-optional lookup.
	const b = /** @type {Record<string, Function | Function[]>} */ (
		componentContext.instance.$$events
	)[/** @type {string} */ (type)];
</script>
