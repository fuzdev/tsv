<script lang="ts">
	// A block comment the author BROKE AFTER, ahead of a should-hug union with an
	// expanded member, binds to the union rather than its first member, so the hug
	// stands: the seam hangs the union after its operator with the comment on its own
	// line, the member expanding below it. Prettier keeps the same binding and, at a
	// type alias `=`, the same form; at `:` and `=>` it leaves the comment trailing the
	// operator and the type flush at the statement's indent.
	let a:
		/* c */
		{
			aaaa: Aaaaaaaaa;
			bbb: Bbbbbb;
			cccccccccccccccccccc: Cccccccccccccccccccccccccccccccccccc;
		} | null;
	interface Single {
		a:
			/* c */
			{
				aaaa: Aaaaaaaaa;
				bbb: Bbbbbb;
				cccccccccccccccccccc: Cccccccccccccccccccccccccccccccccccc;
			} | null;
	}
	type B = () =>
		/* c */
		{
			aaaa: Aaaaaaaaa;
			bbb: Bbbbbb;
			cccccccccccccccccccc: Cccccccccccccccccccccccccccccccccccc;
		} | null;
	type C =
		/* c */
		{
			aaaa: Aaaaaaaaa;
			bbb: Bbbbbb;
			cccccccccccccccccccc: Cccccccccccccccccccccccccccccccccccc;
		} | null;

	// A short member collapses the comment back onto the operator in both formatters
	let d: /* c */ { aaaa: Aaaaaaaaa; bbb: Bbbbbb } | null;
</script>
