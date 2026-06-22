<script lang="ts">
	// 100 chars - stays inline
	interface A {
		method(param: GenericType<'arg1AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA', Arg2, 'arg3', 'arg4'>): void;
	}

	// 101 chars - params break to multiple lines
	interface B {
		method(
			param_: GenericType<'arg1AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA', Arg2, 'arg3', 'arg4'>
		): void;
	}
</script>
