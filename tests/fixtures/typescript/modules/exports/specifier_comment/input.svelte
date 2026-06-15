<script>
	// grouped own-line leading comments (local export)
	export {
		// group 1
		a,
		b,
		// group 2
		c,
		d,
	};

	// grouped own-line leading comments (re-export)
	export {
		// group 1
		e,
		f,
	} from './a';

	// own-line block comment between specifiers (expands)
	export {
		g,
		/* c */
		h,
	};

	// own-line line comment after last specifier
	export {
		i,
		// after last
	} from './b';

	// trailing line + own-line line comments after last specifier
	export {
		j, // c1
		// c2
	};
</script>
