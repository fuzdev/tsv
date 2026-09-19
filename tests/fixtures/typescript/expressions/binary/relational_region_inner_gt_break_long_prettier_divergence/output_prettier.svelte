<script lang="ts">
	// A lone `>` that ends a line is where tsc settles a type-argument list: its recovering
	// parse runs from an earlier `<` to the first token it cannot take, and a line break past a
	// `>` there commits the region. So the one break this `>` takes is the one AHEAD of it.

	// The shelled operand's line is exactly 100, and stays flat.
	const i1 =
		x <
		(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa > bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb);

	// One character longer: 101, so the operand breaks — ahead of its `>`, never after it.
	const i2 =
		x <
		(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa >
			bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb);

	// No shell at all: a comma is a type-argument separator, so a sibling `>` closes the region
	// an earlier argument's `<` opened. Exactly 100 stays flat.
	fn(
		x < q,
		aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa > bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
	);

	// 101 breaks ahead of the `>`.
	fn(
		x < q,
		aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa >
			bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
	);

	// The pair around a kept-shell chain answers the OUTER `>` alone, so that one still ends its
	// line; the inner one is the first the region reaches, and leads its own.
	const s1 =
		x <
		(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa >
			bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb) >
		c;

	// An arrow body and an array head reach the same `>` through a function type and a tuple.
	const s2 =
		x <
		(() =>
			aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa >
			bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb);
	const s3 =
		x <
		[
			aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa >
				bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
		];

	// A `<<` opens the region as a `<` does: tsc re-scans it to one where it looks for type
	// arguments.
	const s4 =
		x <<
		(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa >
			bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb);

	// A conditional's branch, an index and an object member reach the `>` through an optional
	// parameter, an indexed access type and a type literal.
	const s5 =
		x <
		(q
			? aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa >
				bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
			: d);
	const s6 =
		x <
		q[
			aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa >
				bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
		];
	const s7 =
		x <
		{
			k:
				aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa >
				bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
		};

	// A block body inside the region is still inside it: the arrow's braces read as a type
	// literal, and `return (` as a method signature.
	const s8 =
		x <
		(() => {
			return (
				aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa >
				bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
			);
		});

	// Where the region has ended, the `>` ends its line as any operator does: a statement's
	// head says nothing about its body, and the `)` of the call the `<` was written in closes it.
	for (let i = 0; i < n; i++) {
		fn(
			aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa >
				bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
		);
	}
	if (x < q)
		fn(
			aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa >
				bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
		);
	fn(x < q)(
		aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa >
			bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
	);
</script>

<!-- The template takes the same rule: svelte-check hands this expression to tsc as written. -->

{#if x < (aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa > bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb)}
	text
{/if}
