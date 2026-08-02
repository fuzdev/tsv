<script lang="ts">
	/* keyword-gap comment at exactly 100 - restores breaking, but the line fits */
	import /* c */ type { AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA } from './mod';

	/* keyword-gap comment at 101 - a comment on the lone specifier restores breaking */
	import /* c */ type {
		BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB
	} from './mod';

	/* `type`-to-brace gap at 101 - the same rule from the other header gap */
	import type /* c */ {
		CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC
	} from './mod';

	/* control: no comment at 101 - a lone specifier on its own never breaks */
	import type { DDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDD } from './mod';
</script>
