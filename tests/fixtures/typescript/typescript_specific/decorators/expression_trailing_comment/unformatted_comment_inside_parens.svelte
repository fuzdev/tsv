<script lang="ts">
	@(a + b /* c1 */)
	class A {}

	@(a[b] /* c2 */)
	class B {}

	@(x! /* c3 */)
	class C {}

	@(a ? b : c // c4
	)
	class D {}
</script>
