<script lang="ts">
	// update operands need parens before a non-null assertion — else `++c!` parses as `++(c!)`
	const r1 = (++n)!;
	const r2 = (n--)!;
	// regression guards — already kept: unary and binary operands
	const r3 = (-n)!;
	const r4 = (typeof x)!;
	const r5 = (x + y)!;
</script>
