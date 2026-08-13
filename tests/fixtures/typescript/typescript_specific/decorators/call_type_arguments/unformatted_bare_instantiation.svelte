<script lang="ts">
	@g<number>()
	class A {}

	@a.b<T>()
	class B {}

	// With no trailing call the decorator is a TSInstantiationExpression, which
	// prettier parenthesizes (it is not a decorator member expression).
	@a.b<number>
	class C {}

	@g<<T>() => void>()
	class D {}
</script>
