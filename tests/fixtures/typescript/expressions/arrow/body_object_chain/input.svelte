<script lang="ts">
	// Arrow body: object literal at the leftmost (no-lookahead) position needs
	// parens so `{` isn't parsed as a block. Parens wrap just the object, not
	// the whole body — matching prettier's startsWithNoLookaheadToken.

	// Computed member access
	const a = (x: string) => ({ a: 'x' })[x];

	// Dot member access
	const b = () => ({ a: 1, b: 2 }).toString();

	// Multiline with computed access
	const c = (x: string) => ({ a: 'x', b: 'y', c: 'z' })[x];

	// Logical operator — object is the left operand
	const g = () => ({ a: 1 }) && z;

	// Arithmetic operator
	const h = () => ({ a: 1 }) + z;

	// Relational operator
	const i = () => ({ a: 1 }) instanceof Z;

	// Conditional — object is the test
	const j = () => ({ a: 1 }) ? z : w;

	// Member access then operator — leftmost object still wraps
	const k = () => ({ a: 1 }).b && z;

	// Call then member access — leftmost object wraps
	const l = () => ({ a: 1 })().c;

	// Postfix update — leftmost object wraps
	const m = () => ({ a: 1 }).b++;

	// Object nested in a sequence inside a larger body — the leftmost object still
	// wraps even though the sequence parens already protect it (matches prettier)
	const n = () => (({ a: 1 }), z) && b;
	const o = () => (({ a: 1 }), z).c;

	// 100 boundary — object + as stays flat (100 chars)
	const d = (x = 'a', y = 'b'): T =>
		({
			a: x as A,
			b: x,
			c: 'value________________________',
			d: `/tmp/${x}`,
			e: { a: 'x', b: y }
		}) as T;

	// 101 boundary — object + as wraps (101 chars)
	const e = (x = 'a', y = 'b'): T =>
		({
			a: x as A,
			b: x,
			c: 'value_________________________',
			d: `/tmp/${x}`,
			e: { a: 'x', b: y }
		}) as T;

	// Control: plain arrow body object (no chain)
	const f = () => ({ a: 1, b: 2 });
</script>
