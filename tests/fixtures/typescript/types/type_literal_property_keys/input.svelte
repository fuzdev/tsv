<script lang="ts">
	type A = {
		// String literals (requires quotes - can't be identifiers)
		'multi-word': number;
		'has space': string;
		'': boolean;
		// Method with string literal name
		'method-name'(): void;

		// Numeric property names
		0: string;
		42: number;

		// Accessor signatures
		get value(): string;
		set value(v: string);
		// Getter only (readonly accessor)
		get items(): readonly string[];
		// Setter only
		set data(v: unknown);
		// Accessor with keyword as name
		get get(): string;
		set set(v: number);
		// Accessor with string/numeric names
		get 'accessor-name'(): string;
		get 1(): number;
	};
</script>
