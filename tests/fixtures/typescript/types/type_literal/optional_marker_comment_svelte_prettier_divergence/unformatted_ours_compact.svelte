<script lang="ts">
	type T = {
		// Comment between ? and : in type-literal property
		a?/* c1 */: number;

		// Comment between ? and ( in type-literal method
		m?/* c2 */(x: number): void;
	};
</script>
