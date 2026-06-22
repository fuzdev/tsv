<script lang="ts">
	// Function type as type arg
	const a = fn<() => void>();
	const b = fn<(x: number) => string>();

	// Function type after comma
	const c = fn<T, () => void>();
	const d = fn<T, (x: number) => string>();

	// Union with function type
	const e = fn<T | (() => void)>();
	const f = fn<T | ((x: number) => string)>();

	// Intersection with function type
	const g = fn<T & (() => void)>();

	// Union with tuple type
	const h = fn<T | [A, B]>();

	// Intersection with object type
	const i = fn<T & { a: number }>();

	// Multiple type args
	const j = fn<T, U, V>();

	// Method call with union type arg
	const k = obj.method<T | null>();

	// Chained calls with type args
	const l = fn<T>().bar<U | V>();

	// Qualified type names
	const m = fn<Ns.Type>();
	const n = fn<A.B.C | D.E>();

	// this type
	const o = fn<this>();

	// Literal types
	const p = fn<'literal'>();
	const q = fn<'>' | T>();

	// Indexed types
	const r = fn<T[K]>();

	// Array types
	const s = fn<T[]>();
	const t = fn<T[] | U>();
</script>
