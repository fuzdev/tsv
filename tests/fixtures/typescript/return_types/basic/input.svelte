<script lang="ts">
	function fn1(): number {
		return 1;
	}
	function fn2(): string {
		return '';
	}
	const arrow1 = (): number => 1;
	const arrow2 = (a: number): string => String(a);
	const obj = {
		method(): void {},
		get x(): number {
			return 0;
		},
	};
	class A {
		method(): number {
			return 0;
		}
	}
</script>
