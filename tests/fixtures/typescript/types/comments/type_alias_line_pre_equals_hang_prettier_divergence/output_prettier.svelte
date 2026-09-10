<script lang="ts">
	// A line comment before `=` drops `= value` onto a continuation line (tsv keeps the
	// comment on the head, prettier relocates it across the `=`). The value's own lines
	// then sit at the `=`'s level, not one deeper — the column prettier gives them once
	// it has relocated the comment, so the divergence is the comment's position alone.
	// Every arm that BREAKS after `=` is here; the arms that HUG it are the null
	// controls below, where a continuation one level in is the correct shape.

	// Union — the control that was already right.
	type A<T> = // c
		| Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
		| Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb;

	// Generic-conditional — prettier's `shouldBreakBeforeConditionalType`.
	type B<T> = // c
		Aaaaaaaaaaaaaaaaaaa<Xxxxxxxxxxxx> extends Bbbbbbbbbbbbbbbbbb<Yyyyyyyyyy>
			? Cccccccccccccccccccccc
			: Dddddddddddddddddddddd;

	// Template-literal type — force-breaks, so it hangs whatever its width.
	type C<T> = // c
		`prefix_${Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa}_${Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb}_suffix`;

	// `fluid` with no reachable break point: the marker breaks after `=`. The value is
	// 95 chars, so it overflows the `=` line and lands exactly on 100 one line down.
	type D<T> = // c
		Aaaaaaaaaaaaaaaa.Bbbbbbbbbbbbbbbb.Cccccccccccccccc.Dddddddddddddddd.Eeeeeeeeeeeeeeeeeeeeeeeeeee;

	// Null control: an intersection HUGS the `=`, so its continuation is one level in.
	type E<T> = // c
		Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa &
			Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb;

	// Null control: a plain conditional hugs the `=` and wraps its branches one in.
	type F<T> = // c
		Xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx extends Yyyyyyyyyyyyyyyyyyyyyyyyyyyyy
			? Zzzzzzzzzzzzzzzzz
			: Wwwwwwwwwwwwww;

	// Null control: complex type params take prettier's `break-lhs` and hug.
	type G<Tttttttttttttt extends string, Uuuuuuuuuuuuu = number> =
		// c
		Ssssssssssssssssssssssssssssssssssssssssssssssssssssssssss;
</script>
