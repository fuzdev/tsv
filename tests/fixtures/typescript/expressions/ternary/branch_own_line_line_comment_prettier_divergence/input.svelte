<script lang="ts">
	// An own-line `//` in a conditional's `?`→consequent gap keeps the line the author gave it;
	// the branch hangs one level in below it.
	const a = cond
		?
			// c1
			b
		: c;

	// ...and in the `:`→alternate gap.
	const d = cond
		? e
		:
			// c2
			f;

	// A branch's clarity parens survive the hang, and so does a comment trailing the branch.
	const g = cond
		?
			// c3
			(h ?? i)
		: j;

	const k = cond
		? l
		:
			// c4
			(m as T); // t

	// A run keeps one comment per line, in order.
	const n = cond
		?
			// c5
			// c6
			o
		: p;

	// A block ahead of the line comment keeps its own line too.
	const q = cond
		? r
		:
			/* c7 */
			// c8
			s;

	// A branch that breaks on its own hangs the same way.
	const t = cond
		?
			// c9
			fn({
				u: 1
			})
		: v;

	// A chained conditional's alternate gap.
	const w = cond1
		? x
		: cond2
			?
				// c10
				y
			: z;

	// Control: a comment the author put ON the operator's line trails it, as before.
	const a1 = cond
		? // c11
			b1
		: c1;
</script>
