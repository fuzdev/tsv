<script lang="ts">
	@x!
	class A {}

	@x.y!
	class B {}

	@x!.y
	class C {}

	// The `!` composes with type arguments, which need no trailing call.
	@x!<number>
	class D {}
</script>
