<script lang="ts">
	// A block comment glued ahead of a should-hug union's first member binds to that
	// member after `as` / `satisfies` too, so the union no longer hugs the keyword: it
	// hangs after it, and once the member overflows takes the one-per-line form with
	// the comment after the pipe.

	// 100 chars stays on one line; past it the union hangs after the keyword, and a
	// hanging line of 101 chars takes the one-per-line form
	x as /* c */ { aaaa: Aaaaaaaaa; bbb: Bbbbbb; cccccccccccccccccccc: Ccccccccccccccccccccc } | null;
	x as
		/* c */ { aaaa: Aaaaaaaaa; bbb: Bbbbbb; cccccccccccccccccccc: Cccccccccccccccccccccccc } | null;
	x as
		| /* c */ { aaaa: Aaaaaaaaa; bbb: Bbbbbb; cccccccccccccccccccc: Ccccccccccccccccccccccccc }
		| null;
	x as
		| /* c */ {
				aaaa: Aaaaaaaaa;
				bbb: Bbbbbb;
				cccccccccccccccccccc: Cccccccccccccccccccccccccccccccccccc;
		  }
		| null;
	x satisfies
		/* c */ { aaaa: Aaaaaaaaa; bbb: Bbbbbb; cccccccccccccccccccc: Ccccccccccccccc } | null;
	x satisfies
		| /* c */ { aaaa: Aaaaaaaaa; bbb: Bbbbbb; cccccccccccccccccccc: Cccccccccccccccccccccccccccccc }
		| null;
</script>
