<script lang="ts">
	// A block comment written before the `;` stays before it
	declare module 'a' /* c */;

	// A run of blocks written on the name's line stays inline
	declare module 'b' /* c1 */ /* c2 */;

	// A line comment can't precede the `;`, so it trails after it
	declare module 'c' // c
	;

	// An own-line comment drops to its own line below the `;`
	declare module 'd'
	/* d */;

	// A blank line the author left above an own-line comment survives
	declare module 'e'

	/* e */;

	declare module 'f';
</script>
