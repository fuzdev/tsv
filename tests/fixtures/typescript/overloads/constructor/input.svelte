<script lang="ts">
	class A {
		a: number;
		b: number;

		constructor();
		constructor(a: number, b: number);
		constructor(c: {a: number; b: number});
		constructor(aOrC?: number | {a: number; b: number}, b?: number) {
			if (typeof aOrC === 'object') {
				this.a = aOrC.a;
				this.b = aOrC.b;
			} else {
				this.a = aOrC ?? 0;
				this.b = b ?? 0;
			}
		}
	}
</script>
