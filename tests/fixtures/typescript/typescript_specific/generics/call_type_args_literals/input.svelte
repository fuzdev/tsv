<script lang="ts">
	// Numeric literal types
	const a = fn<42>();
	const b = fn<0xff>();
	const c = fn<1e10>();
	const d = fn<1_000_000>();
	const e = fn<100n>();

	// Negative numeric literals
	const f = fn<-1>();
	const g = fn<-0.5>();

	// Template literal types
	const h = fn<`hello`>();
	const i = fn<`a${T}b`>();

	// Combined with other type args
	const j = fn<T, 42>();
	const k = fn<42, U>();
	const l = fn<42 | 43>();
	const m = fn<-1 | 0 | 1>();

	// Method calls
	const n = obj.method<42>();
	const o = obj.method<`test`>();

	// New expression
	const p = new A<42>();
	const q = new A<`template`>();
</script>
