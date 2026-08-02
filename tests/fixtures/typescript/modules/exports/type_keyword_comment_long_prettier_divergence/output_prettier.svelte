<script lang="ts">
	/* keyword-gap comment at exactly 100 - restores breaking, but the line fits */
	export type { /* c */ AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA } from './mod';

	/* keyword-gap comment at 101 - a comment on the lone specifier restores breaking */
	export type {
		/* c */ BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB
	} from './mod';

	/* `type`-to-brace gap at 101 - the same rule from the other header gap */
	export type {
		/* c */ CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC
	} from './mod';

	/* control: no comment at 101 - a lone specifier on its own never breaks */
	export type { DDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDD } from './mod';
</script>
