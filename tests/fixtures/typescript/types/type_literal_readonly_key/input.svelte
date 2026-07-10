<script lang="ts">
	type T = {
		// `readonly` modifier + string / number literal key
		readonly 'a-b': A;
		readonly 1: B;
		// `readonly` as the member name — a method named `readonly`
		readonly(): void;
	};
</script>
