<script lang="ts">
	// line comment trailing re-export `{` - single specifier
	export { // c1
		a
	} from './a';

	// line comment trailing re-export `{` - multiple specifiers
	export { // c2
		b,
		c
	} from './b';

	// line comment trailing export `{` (no source)
	const d = 1;
	export { // c3
		d
	};

	// block comment trailing `{`, then own-line leading comment
	export { /* brace */
		// lead
		e
	} from './e';

	// an author blank BELOW the pulled comment survives — the blank is authorship, not
	// the container's leading gap
	export { // c4

		f
	} from './f';

	// the same, with the block spelling (an own-line comment is what pulls a block onto
	// the `{` line)
	export { /* brace2 */

		// lead
		g
	} from './g';

	// the blank ABOVE the comment is a different question and stays erased — it sits
	// against the delimiter, where both formatters drop it at every bracket
	export {
		// c5
		h
	} from './h';
</script>
