<script lang="ts">
	// non-decimal, separated and bigint keys on interface members decode like
	// standalone literals
	interface A {
		0x10: string;
		1_0: number;
		0b11: boolean;
		0o7: symbol;
		1n: null;
		1e3: undefined;
		0.5: never;
	}

	// the same reader on a type literal's property, method and readonly members
	type B = {
		0xb_b: string;
		2n(): void;
		readonly 0b1_1: number;
	};
</script>
