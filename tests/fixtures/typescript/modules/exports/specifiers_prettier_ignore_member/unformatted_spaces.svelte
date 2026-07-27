<script lang="ts">
	const aaa   =   1;
	const ccc = 2;
		const ddd = 3;

	// an own-line directive after the `{` freezes only the first specifier
	export {
		// prettier-ignore
		aaa   as   a1,
			ccc   as   c1
	};

	// an own-line block comment behaves identically
	export   {
		/* prettier-ignore */
		ddd   as   d1
	};

	// a re-export from a source
	export {
		// prettier-ignore
		eee   as   e1,
		  fff   as   f1
	} from   './a';

	// an inline `type` modifier rides inside the frozen slice
	export {
		// prettier-ignore
		type   G,
			hhh   as   h1
	} from './b';

	// a type-only export
	export   type   {
		// prettier-ignore
		III   as   I1,
		  JJJ   as   J1
	} from './c';
</script>
