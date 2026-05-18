<script lang="ts">
	// method - comment with parens
	type A = {
		a /* a(b) */(): void;
	};

	// property - comment with parens
	type B = {
		a /* a(b) */: number;
	};
</script>
