<script lang="ts">
	/* lone re-export specifier - over print width, stays on one line */
	export { aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa } from './mod';

	/* lone renamed re-export - over print width, stays on one line */
	export { bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb as cccccccccccccccccccccccccccccccccccccccc } from './mod';

	/* lone type-only re-export - over print width, stays on one line */
	export type { DDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDD } from './mod';

	/* lone per-specifier type modifier - over print width, stays on one line */
	export { type eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee as ffffffffffffffffffffffffffffffffffffffff } from './mod';

	/* lone local export, no `from` - over print width, stays on one line */
	export { ggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggg };

	/* two specifiers at exactly 100 - stays inline */
	export { hhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhh, iiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiii } from './mod';

	/* two specifiers at 101 - a second specifier restores breaking */
	export {
		jjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjj,
		kkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkk
	} from './mod';
</script>
