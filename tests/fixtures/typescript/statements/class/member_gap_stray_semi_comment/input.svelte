<script>
	class A {
		// A comment before a stray `;` binds inside its own slot and trails.
		a = 1; /* c1 */
		b = 2;

		// A comment after the stray `;` leads the next member.
		c = 3;
		/* c2 */ d = 4;

		// One on each side: the stray `;` splits the gap.
		e = 5; /* c3 */
		/* c4 */ f = 6;
	}
</script>
