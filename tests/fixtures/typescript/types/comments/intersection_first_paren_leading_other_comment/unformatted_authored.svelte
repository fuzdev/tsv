<script lang="ts">
	// the first member's REDUNDANT parens are stripped, so its leading line comment hoists
	// out of the intersection; a line comment between later members must be preserved, not
	// dropped
	type T1 = [
		(// h1
		a) & b // c1
		& d
	];
</script>
