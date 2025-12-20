<script lang="ts">
	class A {
		fn(x: string): object;
		fn(x: Buffer): object;
		fn(x: string | Buffer): object {
			return {};
		}
	}

	class B {
		fn(x: string): number;
		fn(x: number): string;
		fn(x: boolean): boolean;
		fn(x: string | number | boolean): string | number | boolean {
			return x;
		}
	}
</script>
