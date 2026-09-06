<script lang="ts">
	// `(c) : d => e` in a consequent is `c` and the ternary's `:` — the would-be arrow
	// `(c): d => e` is not followed by a second `:`, so the `:` cannot annotate it
	const s1 = b ? (c) : d => e;

	// The same one level down: `({ y }) : z => …` inside the consequent arrow's body
	const s2 = x ? y => ({ y }) : z => ({ z });

	// A consequent arrow followed by the ternary's `:` keeps its return type, while its
	// own body `(d) : e => f` is the plain `d`: after `f` no second `:` follows
	const s3 = a
		? (b): c =>
			(d)
		: e => f;

	// A nested ternary's alternate `(d) : e => f`: the `:` is the outer ternary's
	const s4 = a ? b ? c : (d) : e => f;

	// A consequent arrow's body `(c): d => e`: the `:` is the ternary's
	const s5 = a ? (b) => (c): d => e;

	// A head that is no parameter list: `(b + c) : d => e` is the consequent `b + c`
	const s6 = a ? (b + c) : d => e;

	// Outside a ternary consequent the `:` is a return type
	const f1 = (b): c => d;
</script>
