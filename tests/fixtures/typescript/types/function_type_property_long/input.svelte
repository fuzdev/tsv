<script lang="ts">
	// 100 chars - fits on one line
	type A = {
		fn_aaa: (options: { aaa: string; bbb: number }) => Promise<{ ccc: string; ddd: number } | null>;
	};

	// 101 chars - params break, return type stays inline (both formatters agree)
	type B = {
		fn_aaaa: (options: {
			aaa: string;
			bbb: number;
		}) => Promise<{ ccc: string; ddd: number } | null>;
	};

	// Multiline return type - params should stay inline (shouldGroupFunctionParameters)
	type C = {
		fn_c: (options: { aaa: string; bbb: number }) => Promise<{
			ccc: string;
			ddd: number;
			eee: number;
		} | null>;
	};
</script>
