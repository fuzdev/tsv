<script>
	// a block comment sharing the `:` line trails it inline
	import a from './a' with { type: /* c */ 'json' };

	// a block comment the author isolated on its own line keeps it, hanging the value
	import b from './b' with {
		type:
			/* c */
			'json'
	};

	// a multiline block comment sharing the `:` line stays inline
	import c from './c' with {
		type: /* c1
		   c2 */ 'json'
	};

	// a multiline block comment on its own line keeps it too
	import d from './d' with {
		type:
			/* c1
			   c2 */
			'json'
	};

	// the own-line comment on a non-first attribute hangs that value only
	import e from './e' with {
		alpha: 'one',
		beta:
			/* c */
			'two'
	};

	// glued blocks are each adjacent to a neighbor, so the run stays inline
	import f from './f' with { type:
		/* c1 */ /* c2 */
		'json' };
</script>
