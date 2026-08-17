<script lang="ts">
	// line comment trailing import `{` - single specifier
	import { // c1
		a
	} from './a';

	// line comment trailing import `{` - multiple specifiers
	import { // c2
		b,
		c
	} from './b';

	// block comment trailing `{`, then own-line leading comment
	import { /* brace */
		// lead
		d
	} from './d';

	// an author blank BELOW the pulled comment survives — the blank is authorship, not
	// the container's leading gap
	import { // c3

		e
	} from './e';

	// the same, with the block spelling (an own-line comment is what pulls a block onto
	// the `{` line)
	import { /* brace2 */

		// lead
		f
	} from './f';

	// the blank ABOVE the comment is a different question and stays erased — it sits
	// against the delimiter, where both formatters drop it at every bracket
	import {
		// c4
		g
	} from './g';
</script>
