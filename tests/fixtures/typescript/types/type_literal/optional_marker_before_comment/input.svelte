<script lang="ts">
	type T = {
		// Property with comment before ?
		a /* c1 */?: number;

		// Method with comment before ?
		m /* c2 */?(x: number): void;
	};
</script>
