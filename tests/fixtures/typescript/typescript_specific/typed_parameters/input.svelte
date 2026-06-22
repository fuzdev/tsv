<script lang="ts">
	function fn1(a: number) {
		return a;
	}
	function fn2(a: number, b: string) {
		return a;
	}
	function fn3(a, b: number) {
		return b;
	}
	function fn4(a: number = 0) {
		return a;
	}
	const arrow1 = (x: number) => x;
	const arrow2 = (x: number, y: string) => x;
	const obj = {
		method(a: number) {
			return a;
		}
	};
</script>
