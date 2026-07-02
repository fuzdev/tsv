<script lang="ts">
	class A {
		static #fn() {}
		static #a = { b: 0 };

		// A private name in the member chain keeps the parens — the bare form
		// `@A.#fn` is valid per the TC39 decorators grammar and normalizes here
		@(A.#fn) m1() {}

		// A call on a chain containing a private name
		@(A.#fn()) m2() {}

		// The chain can continue past the private name
		@(A.#a.b) m3() {}
	}
</script>
