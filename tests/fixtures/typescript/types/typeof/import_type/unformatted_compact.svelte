<script lang="ts">
	// typeof import in generic call type argument
	const a=fn<typeof import('./a')>();
	const b=fn<typeof import('./b').T>();
	const c=fn<typeof import('./c').A.B>();

	// typeof import with type arguments on the import
	const d=fn<typeof import('./d').Type<string>>();

	// Multiple type args including typeof import
	const e=fn<T,typeof import('./e')>();
	const f=fn<typeof import('./f'),U>();

	// Union with typeof import
	const g=fn<T|typeof import('./g')>();

	// Intersection with typeof import
	const h=fn<T&typeof import('./h')>();

	// Method call with typeof import type arg
	const i=obj.method<typeof import('./i')>();

	// New expression with typeof import type arg
	const j=new A<typeof import('./j')>();

	// Await with generic that uses typeof import
	const k=await fn<typeof import('./k')>();
</script>
