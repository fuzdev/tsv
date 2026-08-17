<script>
	// a block after the comma stays on the comma line, in front of the line comment the
	// author wrote after it
	import { a1,/* c1 */// c2
b1 } from './a';

	// a run of blocks after the comma keeps its order
	import { a2,/* c1 *//* c2 */// c3
b2 } from './b';

	// exports take the same placement
	export { a3,/* c1 */// c2
b3 };

	// a block with no line comment after it leads the next specifier instead
	import { a4,/* c1 */b4 } from './c';
</script>
