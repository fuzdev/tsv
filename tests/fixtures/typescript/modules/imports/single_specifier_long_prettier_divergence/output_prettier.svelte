<script lang="ts">
	/* single named specifier - 98 content chars (visual 100, doesn't wrap) */
	import { aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa } from './mod';

	/* single named specifier - 99 content chars (visual 101, we wrap) */
	import { aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa } from './mod';

	/* single named specifier with alias - 98 content chars (visual 100, doesn't wrap) */
	import { aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa as bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb } from './mod';

	/* single named specifier with alias - 99 content chars (visual 101, we wrap) */
	import { ccccccccccccccccccccccccccccccccccc as ddddddddddddddddddddddddddddddddddd } from './mod';

	/* single type specifier - 98 content chars (visual 100, doesn't wrap) */
	import type { AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA } from './mod';

	/* single type specifier - 99 content chars (visual 101, we wrap) */
	import type { BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB } from './mod';
</script>
