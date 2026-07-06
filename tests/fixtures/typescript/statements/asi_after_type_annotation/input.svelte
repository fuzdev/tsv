<script lang="ts">
	// Postfix bracket after a type on a new line: ASI splits into a new statement
	let a: number;
	[0];

	type T = number;
	[e2];

	// Same line stays an indexed-access type (no ASI)
	let b: A[K];
</script>
