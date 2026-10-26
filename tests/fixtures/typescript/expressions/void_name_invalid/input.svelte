<script lang="ts">
	// `void` is an operator, never a name: parenthesized, its expression is a primary the
	// `new` callee, a decorator and a heritage clause each take; bare, `void` is a syntax
	// error at every one of them (see input_invalid_*)
	new (void 0)();
	@(void 0)
	class A {}
	class B {
		@(void 0) m() {}
	}
	class C extends (void 0) {}

	// the type keywords that are not reserved words stay names there
	new number();
	@number
	class D {}
	class E {
		@number m() {}
	}
	class F extends string {}
</script>
