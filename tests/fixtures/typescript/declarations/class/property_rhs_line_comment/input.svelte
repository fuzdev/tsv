<script lang="ts">
	class A {
		b: string = // comment
			c;

		d = // comment
			e;

		// line comment then an own-line block and a glued multiline block stay on
		// consecutive lines (no blank between them)
		f = // comment
			/* f1 */
			/* f2
			 */ g;

		// a glued block run stays glued even when a line comment forced the break
		h = // comment
			/* h1 */ /* h2
			 */ i;
	}
</script>
