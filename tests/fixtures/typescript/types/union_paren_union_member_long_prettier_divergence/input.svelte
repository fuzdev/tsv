<script lang="ts">
	// Short paren-union member stays inline
	type Short = Z | (A | B);

	// Inner paren-union at exactly 100 - stays inline (no divergence)
	type Fit =
		| Z
		| (Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa | Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb);

	// Inner paren-union at 101 - breaks one member per line; closing `)` at the offset
	type Brk =
		| Z
		| (
				| Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
				| Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
			);
</script>
