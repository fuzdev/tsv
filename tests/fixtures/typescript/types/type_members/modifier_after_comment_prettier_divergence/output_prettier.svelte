<script lang="ts">
	interface A {
		// Comment between ? and : in interface property
		a /* c1 */?: number;

		// Comment between ? and ( in interface method
		b?(/* c2 */ x: number): void;
	}
</script>
