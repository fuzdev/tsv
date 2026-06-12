<script lang="ts">
	// Index signature with intersection TypeLiteral value
	type T = {
		[key: string]: A & {
b: B;
		};
	};

	// Multiple properties
	type U = {
		[k: number]: Base & {
a: A;
b: B;
		};
	};
</script>
