<script lang="ts">
	// Short members - own-line comment between them, no break (matches prettier)
	type Short =
		| A
		// c
		| B;

	// Member at exactly 100 - stays inline, no internal break (matches prettier)
	type Fit =
		| Outer<Inner<Entity, 'aaaaaa' | 'bbbbbb' | 'cccccc' | 'dddddd' | 'eeeeeeeeeeeeeeeeeeeeeeeeee'>>
		// c
		| B;

	// Member at 101 - breaks the outer generic; inner content indents one level past the
	// `| ` offset and the closing `>` sits at the offset (tabs-only: prettier uses spaces)
	type Brk =
		| Outer<
				Inner<Entity, 'aaaaaa' | 'bbbbbb' | 'cccccc' | 'dddddd' | 'eeeeeeeeeeeeeeeeeeeeeeeeeee'>
			>
		// c
		| B;
</script>
