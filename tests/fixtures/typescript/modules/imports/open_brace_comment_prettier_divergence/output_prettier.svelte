<script lang="ts">
	// line comment trailing import `{` - single specifier
	import {
		// c1
		a,
	} from './a';

	// line comment trailing import `{` - multiple specifiers
	import {
		// c2
		b,
		c,
	} from './b';

	// block comment trailing `{`, then own-line leading comment
	import {
		/* brace */
		// lead
		d,
	} from './d';
</script>
