<script>
	// inline block comment — parens preserved for comment
	const a = () => (x /* c */);

	// curried arrow — outer parens preserved for comment
	const b = (a) => ((b) => z /* c */);

	// multiple levels — each level preserves parens
	const c = (a) => ((b) => ((c) => w /* c1 */) /* c2 */);

	// line comment — forces multiline parens
	const d = () => (
		x // c
	);

	// own-line block comment — forces multiline parens
	const e = () => (
		x
		/* c */
	);

	// inside call arg — parens preserved
	f(() => (x /* c */));
</script>
