<script lang="ts">
	// A redundant leading `|` or `&` on a type argument is stripped, in expression
	// position (`fn<...>()`, `new Foo<...>()`) as well as type position.
	type A = Foo<T>;
	type B = Foo<T | U>;
	type C = Foo<T & U>;
	const a = fn<T>();
	const b = fn<T | U>();
	const c = new Foo<T | U>();
	const d = fn<T & U>();
</script>
