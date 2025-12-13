<script lang="ts">
	// Readonly index signature in type
	type ReadonlyDict = {
		readonly [key: string]: number;
	};

	// Readonly index signature with properties
	type ReadonlyMixed = {
		readonly [key: string]: unknown;
		readonly name: string;
	};

	// Readonly in interface
	interface ReadonlyInterface {
		readonly [key: string]: number;
	}

	// Readonly number index
	type ReadonlyArray = {
		readonly [idx: number]: string;
		readonly length: number;
	};
</script>
