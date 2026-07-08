<script lang="ts">
	class A {
		// comment between @ and the decorator expression
		fn1(@/* c */ dec a) {}

		// inline block comment after a plain param decorator
		fn2(@dec /* c */ b) {}

		// inline block comment after a param-property decorator
		constructor(@dec /* c */ readonly c) {}

		// block comment between two decorators
		fn3(@dec1 /* c */ @dec2 d) {}

		// own-line comment between an own-line decorator and the binding
		fn4(
			@dec
			/* c */
			e
		) {}

		// line comment trailing a decorator
		fn5(
			@dec // c
			f
		) {}
	}
</script>
