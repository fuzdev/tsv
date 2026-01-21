<script lang="ts">
	// 100 chars - stays inline
	interface A {
		new (param__: GenericType<'arg1AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA', Arg2, 'arg3', 'arg4'>): void;
	}

	// 101 chars - params break to multiple lines
	interface B {
		new (
			param___: GenericType<'arg1AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA', Arg2, 'arg3', 'arg4'>,
		): void;
	}
</script>
