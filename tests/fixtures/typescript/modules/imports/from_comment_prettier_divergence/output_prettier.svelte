<script lang="ts">
	// default binding, block comment before from
	import Foo /* c1 */ from './a';

	// default binding, line comment before from
	import Bar from './b'; // c2

	// namespace binding, block comment before from
	import * as ns1 /* c3 */ from './c';

	// namespace binding, line comment before from
	import * as ns2 from './d'; // c4

	// named specifiers, block comment before from
	import {a /* c5 */} from './e';

	// named specifiers, line comment before from
	import {
		b, // c6
	} from './f';
</script>
