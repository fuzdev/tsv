<script lang="ts">
	// Class implements: a line comment after the keyword hangs the items; a block
	// stranded after a comma (`A, /* c */⏎B`) hugs the next item once the list fits
	class Class
		implements // c
			A, /* c */ B, C {}

	// Interface extends: the same keyword-hang seam
	interface Interface
		extends // c
			A, /* c */ B, C {}

	// A multi-line block after the keyword hangs the items the same way
	class Block
		implements /* a
		b */
			A, /* c */ B, C {}

	// The list wraps: each item on its own line, the stranded block stays on its comma line
	class Long
		implements // c
			SomeVeryLongBaseInterfaceNameHere, /* c */
			AnotherVeryLongInterfaceNameToo,
			YetAnotherLongInterfaceName {}
</script>
