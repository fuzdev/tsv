<script lang="ts">
	// source-multiline object head arg stays broken, so ALL args break out -
	// the hug states can't hold a head arg that already breaks
	const a = obj.aa(x).bb(
		{
			a: 1,
			b: 2
		},
		(y) => {
			return y;
		}
	);

	// contrast: a flat object head arg stays inline and the block arrow hugs
	const b = obj.aa(x).bb({ a: 1 }, (y) => {
		return y;
	});
</script>
