<script lang="ts">
	class A {
		fn1(@dec { a, b }: T) {}

		fn2(@dec [a, b]: T) {}

		fn3(@dec1 @dec2 { a }: T) {}

		fn4(@dec { a } = {}) {}
	}
</script>
