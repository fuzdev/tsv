<script lang="ts">
	// a bare angle-bracket assertion as the left operand of `**`: tsc rejects it (as it does a
	// bare unary there), acorn-typescript reads the assertion over the whole exponentiation,
	// which `expected_svelte.json` records
	let a = <T>x ** 2;
	let b = <T>x! ** 2;
	let c = <T>(x) ** 2;
</script>
