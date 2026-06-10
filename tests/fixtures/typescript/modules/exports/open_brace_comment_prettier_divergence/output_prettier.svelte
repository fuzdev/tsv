<script lang="ts">
	// line comment trailing re-export `{` - single specifier
	export {
		// c1
		a,
	} from './a';

	// line comment trailing re-export `{` - multiple specifiers
	export {
		// c2
		b,
		c,
	} from './b';

	// line comment trailing export `{` (no source)
	const d = 1;
	export {
		// c3
		d,
	};

	// block comment trailing `{`, then own-line leading comment
	export {
		/* brace */
		// lead
		e,
	} from './e';
</script>
