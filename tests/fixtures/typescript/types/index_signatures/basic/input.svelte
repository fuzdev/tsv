<script lang="ts">
	// Index signature in type literal
	type Dict = {
		[key: string]: number;
	};

	// Index signature with number key
	type NumDict = {
		[idx: number]: string;
	};

	// Index signature with symbol key
	type SymDict = {
		[s: symbol]: unknown;
	};

	// Index signature with readonly
	type ReadonlyDict = {
		readonly [key: string]: number;
	};

	// Index signature mixed with properties
	type Mixed = {
		name: string;
		[key: string]: unknown;
	};

	// Index signature in variable type annotation
	const a: {[k: string]: number} = {};

	// Index signature in function parameter
	function fn(obj: {[key: string]: unknown}): void {}

	// Index signature in return type
	function getDict(): {[key: string]: string} {
		return {};
	}
</script>
