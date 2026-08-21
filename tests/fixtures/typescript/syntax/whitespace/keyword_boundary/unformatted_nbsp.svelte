<script lang="ts">
	// A lookahead's identifier scan ends at every ECMAScript `WhiteSpace` and
	// `LineTerminator` character, not just `[ \t\n\r]`, so a keyword followed by
	// one is still a keyword — the `unformatted_*` variants carry NBSP, ZWNBSP and
	// the `Zs` category in each keyword gap below, and both formatters normalize
	// them to the plain spaces here.
	//
	// The `Kµ` / `Aµ` names are the counter-case: U+00B5 shares NBSP's `0xC2` lead
	// byte and is an ordinary identifier character, so the scan crosses it.
	type M<T> = { [K in keyof T]: T[K] };
	const a = f<T extends string ? 1 : 2>(1);
	const b = f<A[B extends C ? 1 : 2]>(1);
	const c = f<new (v: T) => R>(1);
	const d = f<abstract new (v: T) => R>(1);
	const g = (v): new () => R => v;
	const h = (v): abstract new () => R => v;
	const i = (v: T): T extends string ? 1 : 2 => v;
	type N<T> = { [Kµ in keyof T]: T[Kµ] };
	const n = f<Aµ | B>(1);
</script>
