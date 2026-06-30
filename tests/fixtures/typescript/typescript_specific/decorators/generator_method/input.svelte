<script lang="ts">
	class A {
		// Simple decorator on a generator method
		@fn
		*a() {}

		// Call-expression decorator on a generator method
		@fn()
		*b() {}

		// Member-access decorator on a generator method
		@fn.nested
		*c() {}

		// Decorator on an async generator method
		@fn
		async *d() {}

		// Decorator on a static generator method
		@fn
		static *e() {}

		// Inline block comment between decorator and generator method
		@fn /* c */ *f() {}
	}
</script>
