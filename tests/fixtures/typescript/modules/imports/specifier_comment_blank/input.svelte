<script lang="ts">
	// blank after a head comment, before the first specifier
	import {
		// c1

		a
	} from './a';

	// blank between two head comments
	import {
		// c2

		// c3
		b
	} from './b';

	// blank after a head run, before the first specifier
	import {
		// c4
		// c5

		c
	} from './c';

	// blank before an own-line comment following a specifier
	import {
		d,

		// c6
		e
	} from './d';

	// blank between two own-line comments following a specifier
	import {
		f,
		// c7

		// c8
		g
	} from './e';

	// head comment, no blank — base form for the dropped-blank variant
	import {
		// c9
		h,
		i
	} from './f';
</script>
