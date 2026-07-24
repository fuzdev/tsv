<script lang="ts">
	// a glued directive in an inline type-argument list is inert
	type A = Foo</* prettier-ignore */ {x:   1}, b>;

	// sole argument, type position and call site
	type B = Foo</* prettier-ignore */ {x:   1}>;
	const t = fn2</* prettier-ignore */ {x:   1}>(arg);
</script>
