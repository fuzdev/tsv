<script>
	// Keywords as import specifiers
	import { object } from './a';
	import { async as fn1 } from './b';
	import { object as obj, async as fn2 } from './c';

	// Keywords as re-export specifiers
	export { object as renamed } from './d';
	export { async as renamed2 } from './e';

	// Keywords as local export alias
	const a = 1;
	export { a as async };

	// Reserved words as re-export alias
	export { x as await } from './f';

	// Reserved words as export alias (any keyword valid in export name position)
	const b = 2;
	const c = 3;
	export { b as const, c as if };

	// Keyword as function name (in block to avoid shadowing import)
	{
		function object(x) {
			return x;
		}
		function async(x) {
			return x;
		}
		object(1);
	}
</script>
