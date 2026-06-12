<script lang="ts">
	// Short nested multi-arg generic - fits on one line (no wrapping)
	type Short = Outer<Inner<{a: A}, {b: B}>>;

	// 100 chars at indent 2 - stays inline (at boundary)
	type T100 = Outer<
		Inner<{a: A; b: A; c: A; d: A; e: A; f: A; g: A; h: A; i: A; j: A; k: A; l: A}, {m: Abcdefghij}>
	>;

	// 101 chars at indent 2 - inner type args break
	type T101 = Outer<
		Inner<
			{a: A; b: A; c: A; d: A; e: A; f: A; g: A; h: A; i: A; j: A; k: A; l: A},
			{m: Abcdefghijk}
		>
	>;

	// 103 chars at indent 2 - function type return type context (matches corpus pattern)
	type Fn = (a: {
		b: string;
		c: string;
	}) => Promise<
		Result<
			{d: string; e: string; f_name: boolean; g_value: boolean},
			{h_msg: string; i_extra: string}
		>
	>;
</script>
