<script lang="ts">
	// 100 chars - fits on one line
	export const fn_aaaaaaaaaaaa = async (): Promise<(a: string, b?: string) => AAAAAAAAAAAAAA> => {};

	// 101 chars - generic type args break, function type stays inline on its own line
	export const fn_aaaaaaaaaaaaa = async (): Promise<
		(a: string, b?: string) => AAAAAAAAAAAAAA
	> => {};
</script>
