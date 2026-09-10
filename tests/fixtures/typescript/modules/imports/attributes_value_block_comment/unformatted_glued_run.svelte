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

	// an INDENTABLE multiline block hangs the value even when it shares the `:` line -
	// the rule the `:` seam takes from every other assignment operator, keyed on the
	// comment's own print shape rather than on where the author left it
	import g from './g' with { type: /** c
	 */ 'json' };

	// ...at the head of a glued run reaching the value
	import h from './h' with { type: /** c
	 */ /* c1 */ 'json' };

	// ...and at its tail
	import i from './i' with { type: /* c1 */ /** c
	 */ 'json' };

	// an indentable block the author isolated keeps its line, and the value drops below
	import j from './j' with { type:
		/** c
		 */
		'json' };

	// the same on a non-first attribute
	import k from './k' with { alpha: 'one', beta: /** c
	 */ 'two' };

	// Contrast: in the key -> `:` gap the comment trails the key and nothing hangs.
	import l from './l' with { type /** c
	 */: 'json' };
</script>
