<script lang="ts">
	// a frozen clause keeps its stripped pair's comment in its own slice
	for (
		;
		;
		// prettier-ignore
		a  +  (b // c1
	)
	) {}

	// so does a frozen operand another follows
	for (
		;
		;
		i++,
			// prettier-ignore
			a  +  (b // c2
	),
			j++
	) {}

	// and a frozen declarator value another follows
	for (
		let i = 0,
			k =
				// prettier-ignore
				a  +  (b // c3
	),
			j = 1;
		;
	) {}

	// and a frozen last operand of the update
	for (
		;
		;
		i++,
			// prettier-ignore
			a  +  (b // c4
	)
	) {}
</script>
