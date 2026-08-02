<script lang="ts">
	/* lone named specifier - over print width, stays on one line */
	import { aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa } from './mod';

	/* lone renamed specifier - over print width, stays on one line */
	import { bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb as cccccccccccccccccccccccccccccccccccccccc } from './mod';

	/* lone type-only specifier - over print width, stays on one line */
	import type { DDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDD } from './mod';

	/* lone per-specifier type modifier - over print width, stays on one line */
	import { type eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee as ffffffffffffffffffffffffffffffffffffffff } from './mod';

	/* lone string-name specifier - over print width, stays on one line */
	import { 'gggggggggggggggggggggggggggggggggggggggg' as hhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhh } from './mod';

	/* two specifiers at exactly 100 - stays inline */
	import { iiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiii, jjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjj } from './mod';

	/* two specifiers at 101 - a second specifier restores breaking */
	import {
		kkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkk,
		llllllllllllllllllllllllllllllllllllll
	} from './mod';

	/* default binding at exactly 100 - stays inline */
	import mmmmmmmm, { nnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnn } from './mod';

	/* default binding at 101 - a leading binding restores breaking */
	import oooooooo, {
		pppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppp
	} from './mod';
</script>
