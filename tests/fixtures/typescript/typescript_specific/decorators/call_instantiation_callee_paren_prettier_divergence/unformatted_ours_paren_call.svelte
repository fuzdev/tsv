<script lang="ts">
	// a decorator calling an instantiation keeps the pair around the instantiation
	@(f<T>)()
	class C {
		@(f<T>)() m() {}
	}
</script>
