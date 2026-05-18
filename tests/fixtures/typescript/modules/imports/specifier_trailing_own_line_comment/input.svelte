<script>
	// own-line line comment after last specifier
	import {
		a,
		// c
	} from './a';

	// same-line + own-line line comments after last specifier
	import {
		b, // c1
		// c2
	} from './b';

	// own-line block comment after last specifier
	import {
		c,
		/* c */
	} from './c';

	// multiple own-line comments after last specifier
	import {
		d,
		// c1
		// c2
	} from './d';

	// blank line preserved between last specifier and own-line comment
	import {
		e,

		// c
	} from './e';
</script>
