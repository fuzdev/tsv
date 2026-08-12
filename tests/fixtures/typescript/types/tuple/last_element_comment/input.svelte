<script lang="ts">
	// block comment trailing the last element is preserved in place
	type T = [
		aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
		bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb /* comment */
	];

	// a multiline block comment forces the tuple open and trails the last element
	type U = [
		A,
		B /* c1
c2 */
	];
</script>
