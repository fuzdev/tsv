<script lang="ts">
	// 100 chars on param line - generic args stay inline
	type A = {
		foo?: {
			receive?: (
				param_abc: GenericType<'arg1AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA', Arg2, 'arg3', 'arg4'>,
			) => void;
		};
	};

	// 101 chars on param line - generic args MUST break
	type B = {
		foo?: {
			receive?: (
				param_abcd: GenericType<
					'arg1AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA',
					Arg2,
					'arg3',
					'arg4'
				>,
			) => void;
		};
	};
</script>
