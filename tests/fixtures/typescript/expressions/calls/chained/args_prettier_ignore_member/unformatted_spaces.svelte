<script lang="ts">
	// a member-chain call's arguments take the chain's forced-expansion layout
		const a = obj.fn(
		// prettier-ignore
		{ a:   1 }
	).b;

	// an inter-argument directive in a chain call
		const b = obj.fn(
			{ a:   1 }  ,
		// prettier-ignore
		{ b:   2 }
	).c;

	// a curried call — the second argument list is a chain call too
		fn()(
		// prettier-ignore
		{ a:   1 },
			() => {
			b();
		}
	);

	// a broken chain keeps the frozen argument verbatim
		const c = obj
			.first(1)
			.second(
			// prettier-ignore
			{ a:   1 }
			)
		.third(  3  );
</script>
