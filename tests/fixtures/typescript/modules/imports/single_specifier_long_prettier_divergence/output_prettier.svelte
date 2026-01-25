<script lang="ts">
	/* single named specifier - 98 content chars (visual 100, doesn't wrap) */
	import {aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa} from './mod';

	/* single named specifier - 99 content chars (visual 101, we wrap) */
	import {aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa} from './mod';

	/* single named specifier with alias - 98 content chars (visual 100, doesn't wrap) */
	import {aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa as bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb} from './mod';

	/* single named specifier with alias - 99 content chars (visual 101, we wrap) */
	import {cccccccccccccccccccccccccccccccccccc as dddddddddddddddddddddddddddddddddddd} from './mod';

	/* single type specifier - 98 content chars (visual 100, doesn't wrap) */
	import type {AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA} from './mod';

	/* single type specifier - 99 content chars (visual 101, we wrap) */
	import type {BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB} from './mod';
</script>
