<script lang="ts">
	// too wide for one line even indented under `=` — the union breaks per member and the
	// glued leading comment binds to the first member, after its `| `
	type A = /* c */ aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa | bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb;

	// exactly 100 chars — stays inline, comment glued before the first member
	type B = /* c */ eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee | fffffffffffffffffffffffffffffffffffff;

	// exactly 101 chars — breaks after `=` only; the union fits indented, comment still glued
	type D = /* c */ gggggggggggggggggggggggggggggggggggggggg | hhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhh;

	// an own-line-authored leading comment stays above the union's first `| `
	type C =
		/* c */
		| iiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiii
		| jjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjj;
</script>
