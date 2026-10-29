<script lang="ts">
	// A leading comment occupies the interpolation whatever the value's kind, an optional
	// chain included: the interpolation breaks at its `${` and `}` around the chain.
	const aaa = `x${/* c1 */ bbb?.ccc(function () {
		return 1;
	})}y`;

	// The same comment hoisted out of a stripped shell at the chain's head.
	const ddd = `x${(/* c2 */ eee)?.fff(function () {
		return 1;
	})}y`;
</script>
