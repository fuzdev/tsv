<script lang="ts">
	// A head with a non-Identifier parameter breaks the whole chain however short it is —
	// each head takes its own line under the `=` (prettier's shouldBreakChain)
	const a =
		({}) =>
		() =>
			test;
	const b =
		(x = 1) =>
		() =>
			test;
	const c =
		([x]) =>
		() =>
			test;
	const d =
		(...rest) =>
		() =>
			test;

	// Type parameters on a head break it too
	const e =
		<T extends A>() =>
		() =>
			test;

	// A return type breaks only when that head also HAS parameters
	const f =
		(x): A =>
		(y) =>
			test;
	const g = (): A => (y) => test;

	// The trigger is asked of EVERY head, not just the first
	const h =
		() =>
		({}) =>
		() =>
			test;

	// A plain identifier parameter is simple — an annotation on it does not change that
	const i = (a) => (b) => test;
	const j = (x: A) => (y) => test;

	// In a call argument the broken chain progressive-indents instead of sharing one indent
	fn(
		'first',
		({}) =>
			() =>
				test
	);
	fn(
		(x = 1) =>
			() =>
			() =>
				test
	);

	// A binaryish operand takes the same progressive shape
	const k =
		cond ??
		(({}) =>
			() =>
				test);

	// A hugging terminal body still hugs the last head — the break denies the ternary its
	// same-line parens (below) but never these
	const l =
		({}) =>
		() => ({ k: 1 });

	// A sequence terminal hugs too — its parens are the point, so shouldAlwaysAddParens is
	// not gated on the break the way the ternary is
	fn(
		'first',
		({}) =>
			() => (a, b)
	);

	// A ternary terminal is the one hugging body the break DOES deny: the parens exist only
	// to hold it on one line, which a stacked chain has already given up
	fn(
		'first',
		({}) =>
			() =>
				test ? 1 : 2
	);
	const m = (a) => (b) => (test ? 1 : 2);
</script>
