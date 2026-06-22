<script>
	// single specifier with trailing line comment
	import {
		a // comment
	} from './a';

	// last specifier trailing line comment
	import {
		b,
		c // comment
	} from './b';

	// all specifiers with trailing line comments
	import {
		d, // comment1
		e // comment2
	} from './c';

	// hardline path (expanding comment) + last trailing line comment
	import {
		// group
		f,
		g // comment
	} from './d';

	// block comment on last specifier (should work - contrast case)
	import { h /* comment */ } from './e';
</script>
