<script lang="ts">
	// 100 chars - stays inline
	type T = {
		fn: (options: { aaaa: Aaaaaaaaa; bbb: Bbbbbb }) => Promise<Result<{ value: booleanABCDEFGHI }>>;
	};

	// 101 chars - object param breaks, Prettier hugs (options: { and expands inside
	type U = {
		fn: (options: {
			aaaa: Aaaaaaaaa;
			bbb: Bbbbbb;
		}) => Promise<Result<{ value: booleanABCDEFGHIJ }>>;
	};
</script>
