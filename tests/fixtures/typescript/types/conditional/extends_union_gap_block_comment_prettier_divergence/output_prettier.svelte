<script lang="ts">
	// A block comment glued ahead of a should-hug union's first member at a
	// conditional's `extends` binds to that member, so the union no longer hugs: it
	// breaks after `extends` and, fitting on the continuation line, stays flat there.
	type A = T extends /* c */ {
		aaaa: Aaaaaaaaa;
		bbb: Bbbbbb;
		ccccccc: Cccccccccccccccccccccccc;
	} | null
		? 1
		: 2;
</script>
