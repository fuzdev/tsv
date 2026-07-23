<script lang="ts">
	// comment authored trailing the previous member (stays trailing in both formatters);
	// the parenthesized inner union is kept, fits inline, no break
	type Fit =
		| Z // c
		| (D | E);

	// inner union breaks one member per line - content indents one level past the `| (`
	// member offset, closing `)` sits at the offset (tabs-only: prettier uses spaces)
	type Brk =
		| Z // c
		| (
				| Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
				| Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
		  );
</script>
