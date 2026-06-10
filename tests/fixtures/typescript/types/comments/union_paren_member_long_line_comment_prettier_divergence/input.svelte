<script lang="ts">
	// inner union fits inline after the comment relocates out - parens kept, no break
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
