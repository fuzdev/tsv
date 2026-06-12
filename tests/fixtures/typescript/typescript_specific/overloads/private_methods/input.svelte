<script lang="ts">
	class A {
		#fn(x: string): string;
		#fn(x: number): number;
		#fn(x: string | number): string | number {
			return x;
		}
	}
</script>
