<script lang="ts">
	// typeof in call type args
	const a = fn<typeof x>();
	const b = fn<typeof A.B>();

	// keyof in call type args
	const c = fn<keyof T>();
	const d = fn<keyof typeof x>();

	// readonly in call type args
	const e = fn<readonly T[]>();
	const f = fn<readonly [T, U]>();

	// unique symbol in call type args
	const g = fn<unique symbol>();
	const h = fn<T | unique symbol>();

	// Combinations
	const i = fn<T, keyof U>();
	const j = fn<typeof x, keyof T>();
	const k = fn<keyof T | keyof U>();
	const l = fn<readonly T[] | U>();

	// Method calls
	const m = obj.method<typeof x>();
	const n = obj.method<keyof T>();

	// Chained
	const o = fn<typeof x>().bar<keyof T>();

	// New expression
	const p = new A<typeof x>();
	const q = new A<keyof T>();
	const r = new A<readonly T[]>();
	const s = new A<unique symbol>();
</script>
