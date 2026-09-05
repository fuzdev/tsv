<script lang="ts">
	// A chain longer than two calls force-expands when any call has a NON-SIMPLE
	// argument (isSimpleCallArgument). A TS cast is not simple — the only wrappers the
	// question is asked through are the chain-element ones (`!` and optional chaining),
	// never `as` / `satisfies` / `<T>` — so a cast argument alone breaks the chain apart.

	// `as` cast argument — not simple, chain force-expands
	const a = obj
		.fn1(x as T)
		.fn2(y)
		.fn3(z);

	// `satisfies` cast argument — same
	const b = obj
		.fn1(x satisfies T)
		.fn2(y)
		.fn3(z);

	// angle-bracket assertion argument — same
	const c = obj
		.fn1(<T>x)
		.fn2(y)
		.fn3(z);

	// a non-null wrapper IS looked through — `x!` is as simple as `x`, chain stays inline
	const d = obj.fn1(x!).fn2(y).fn3(z);

	// so is optional chaining
	const e = obj.fn1(x?.y).fn2(y).fn3(z);

	// plain identifier argument — chain stays inline
	const f = obj.fn1(x).fn2(y).fn3(z);

	// only three-or-more calls ask the question — a two-call chain keeps its cast inline
	const g = obj.fn1(x as T).fn2(y);
</script>
