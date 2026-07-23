<script lang="ts">
	class C {
		x = 1;
		y;
		fn = () => 1;
		count: number = 5;
		optional?: number;
		static total = 3;
		// Non-identifier keys stay quoted in both formatters.
		'0a' = 6;
		'x-y' = 7;
		'0' = 8;
	}
</script>
