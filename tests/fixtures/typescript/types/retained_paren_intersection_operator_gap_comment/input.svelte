<script lang="ts">
	// block comment before the `&` in a retained paren intersection with a trailing object
	type A1 = (a /* c */ & { x: X }) | c;

	// block comment after the `&`, ahead of the trailing object
	type A2 = (a & /* c */ { x: X }) | c;

	// the same gap where the trailing object is empty
	type A3 = (a & /* c */ {}) | c;

	// a three-member intersection — the gaps between the earlier members take a comment too
	type A4 = (a /* c */ & b & { x: X }) | c;
	type A5 = (a & b /* c */ & { x: X }) | c;

	// the same shell as an optional tuple element
	type A6 = [(a & /* c */ { x: X })?];
</script>
