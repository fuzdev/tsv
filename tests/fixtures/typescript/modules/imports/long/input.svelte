<script lang="ts">
	/* short named import - stays inline (under 100 chars) */
	import { aaaa, bbbb, cccc, dddd, eeee, ffff, gggg, hhhh } from 'short';

	/* long named import - wraps (exceeds 100 chars) */
	import {
		iiiiiiiiii,
		jjjjjjjjjj,
		kkkkkkkkkk,
		llllllllll,
		mmmmmmmmmm,
		nnnnnnnnnn,
		oooooooooo
	} from 'module';

	/* long import with renaming - wraps */
	import {
		ppp as p1,
		qqq as q1,
		rrr as r1,
		sss as s1,
		ttt as t1,
		uuu as u1,
		vvv as v1,
		www as w1
	} from 'rn';

	/* combined import (default + named) - wraps when long */
	import x, {
		xxxxxxxxxx,
		yyyyyyyyyy,
		zzzzzzzzzz,
		aaaaaaaaab,
		bbbbbbbbbc,
		cccccccccd
	} from 'combined';

	/* short type import - stays inline (exactly 100 chars) */
	import type {
		AAAAAAAAAA,
		BBBBBBBBBB,
		CCCCCCCCCC,
		DDDDDDDDDD,
		EEEEEEEEEE,
		FFFFFFFFFF
	} from 'types';

	/* long type import - wraps (exceeds 100 chars) */
	import type {
		GGGGGGGGGG,
		HHHHHHHHHH,
		IIIIIIIIII,
		JJJJJJJJJJ,
		KKKKKKKKKK,
		LLLLLLLLLL,
		MMMMMMMMMM
	} from 'longtypes';

	/* short mixed type/value import - stays inline */
	import { type NNNNNNNNNN, type OOOOOOOOOO, pppppppppp, qqqqqqqqqq, rrrrrrrrrr } from 'mixed';

	/* long mixed type/value import - wraps */
	import {
		type PPPPPPPPPP,
		type QQQQQQQQQQ,
		type RRRRRRRRRR,
		type SSSSSSSSSS,
		tttttttttt,
		uuuuuuuuuu
	} from 'longmixed';
</script>
