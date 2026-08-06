<script>
	import {
		a,
		/* c */
		b
	} from './a';

	export {
		c,
		/* c */
		d
	};

	export {
		e,
		/* c */
		f
	} from './b';

	// A same-line block comment does NOT force expansion
	import {g,
/* c */h} from './c';
</script>
