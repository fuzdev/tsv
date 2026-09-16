<script lang="ts">
	// A relational chain keeps a paren pair around its `<` operand whenever the region
	// between the `<` and the `>` would parse as a type-argument list. Bare, the printed
	// chain re-lexes as a call with type arguments — a different program — because the
	// author's line break before the `[` is the only thing holding the index to an
	// expression, and the formatter folds that break away.
	const a1 = (fn < A[T]) > (t, u);
	const a2 = (fn < A[0.5]) > (t, u);

	// The tail past the would-be closing `>` is not the axis: a sequence, a tagged
	// template, and a lone arrow argument all re-lex alike.
	const a3 = (fn < A[T]) > `x`;
	const a4 = (fn < A[T]) > ((t) => t);

	// Neither is the callee shape — a member and a `this` member ask the same question.
	const a5 = (a.b < A[T]) > (t, u);
	const a6 = (this.b < A[T]) > (t, u);

	// A `{` or `[` head opens the region whatever its body holds, so the pair stands on
	// every one. A tuple, a type literal and a computed key ARE types, and a rest element
	// is a tuple's own; an object spread and an update expression are not, and tsc reads
	// those two as the chain they are — but the same bytes are a type-argument list to a
	// parser that grades no body, which is what tsv's own is here, so a bare output would
	// be one tsv cannot reparse.
	const c1 = (a < [1, 2]) > c;
	const c2 = (a < { x: B }) > c;
	const c3 = (a < { [k]: 1 }) > c;
	const c4 = (a < [...s]) > c;
	const c5 = (a < { ...s }) > c;
	const c6 = (a < [b, c++]) > c;

	// A parenthesized type is a head too, and its own shell survives only where
	// precedence needs it.
	const c7 = (a < (A | B)) > c;

	// A non-null `!` is a type to tsc (`JSDocNonNullableType`), prefix and postfix, and
	// reads the same through a member, an index, and a shell the printer strips.
	const c8 = (a < b!) > c;
	const c9 = (a < b!!) > c;
	const c10 = (a < b.c!) > c;
	const c11 = (a < B[c]!) > c;
	const c12 = (a < !b) > c;

	// An operand that is no type keeps the chain bare: arithmetic, a `this` member, and
	// a name that merely begins with a type keyword.
	const b1 = x < 1 + 2 > (t, u);
	const b2 = x < this.a > (t, u);
	const b3 = p < string$ ? q : r > (t, u);

	// A call, and a function or class expression, are no types either.
	const b4 = x < f() > c;
	const b5 = x < function () {} > c;
	const b6 = x < class {} > c;
</script>
