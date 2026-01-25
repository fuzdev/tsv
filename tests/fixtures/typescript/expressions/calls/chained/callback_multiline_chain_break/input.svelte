<script lang="ts">
	// Chain with callback before map - multiline object forces chain break
	const a = items
		.filter((x) => x.a)
		.map((x) => ({
			a: x.a,
		}));

	// Chain with callback before callback - multiline body forces chain break
	const b = items
		.filter((x) => x.a)
		.map((x) => {
			return x.a;
		});

	// Longer chain - still breaks when multiline callback present
	const c = items
		.filter((x) => x.a)
		.map((x) => ({
			a: x.a,
			b: x.b,
		}))
		.sort();

	// Non-callback method before callback - chain stays inline
	const d = items.slice(0, 10).map((x) => ({
		a: x.a,
	}));

	// Single method with multiline callback - no chain to break
	const e = items.map((x) => ({
		a: x.a,
	}));
</script>
