<script lang="ts">
	// 100 chars - stays inline
	interface A {
		(param___: GenericType<'arg1AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA', Arg2, 'arg3', 'arg4'>): void;
	}

	// 101 chars - params break to multiple lines
	interface B {
		(
			param____: GenericType<'arg1AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA', Arg2, 'arg3', 'arg4'>
		): void;
	}
</script>
