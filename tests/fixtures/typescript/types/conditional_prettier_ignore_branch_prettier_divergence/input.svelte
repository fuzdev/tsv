<script lang="ts">
	// an own-line directive above a branch line stays own-line and freezes that
	// branch's child only
	type A = X extends string
		// prettier-ignore
		? {x:   1}
		: { y: 2 };

	// false branch: own-line above the `:` line
	type B = X extends string
		? { x: 1 }
		// prettier-ignore
		: {y:   2};

	// composite branch: the whole branch freezes, operators and all
	type C = X extends string
		// prettier-ignore
		? {x:   1} | {z:   3}
		: { y: 2 };

	// control: a plain comment keeps the trailing relocation
	type D = X extends string // c
		? { x: 1 }
		: { y: 2 };
</script>
