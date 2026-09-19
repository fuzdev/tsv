<script lang="ts">
	// an angle-bracket assertion as the left operand of `**` keeps its pair: bare, tsc rejects
	// the line and acorn-typescript reads the assertion over the whole exponentiation
	let a = (<T>x) ** 2;
	let b = (<T>--x) ** 2;
	let c = (<T>x!) ** 2;
	let d = (<T>x.y) ** 2;
	let e = (<T>(<U>x)) ** 2;
	let f = 2 ** ((<T>x) ** 2);

	// as the right operand, or ahead of a looser operator, the assertion stays bare
	let g = 2 ** <T>x;
	let h = <T>x * 2;
</script>
