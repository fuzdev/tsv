<script lang="ts">
	// fits at 100 columns: the interpolation stays on one line
	const vvv = `x${(/* c1 */ fff)(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa)
}y`;

	// 101 columns: the interpolation breaks at its `${` and `}`, the comment leading the value
	const www = `x${(/* c2 */ ggg)(bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb)
}y`;
</script>
