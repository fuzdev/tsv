<script lang="ts">
	// Reserved keyword 'new' as property name
	const a: { new: string; delete: number } = { new: 'value', delete: 42 };

	// Optional and readonly with 'new' as property name
	type A = { new?: string };
	type B = { readonly new: string };
	type C = { readonly new?: string };

	// Construct signatures still work
	type D = { new (): void };
	type E = { new <T>(): T };

	// Mixed: property and construct signature with same keyword
	type F = {
		new: string;
		new (): void;
	};

	// Method signatures with reserved keyword names
	type G = { delete(x: string): void };

	// Other contextual keywords as property names
	type H = { get: string; set: number; async: boolean; static: string };

	// Declaration keywords as property names
	type I = { import: string; export: number; extends: boolean; implements: string };
</script>
