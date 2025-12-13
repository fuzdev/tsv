<script lang="ts">
	declare namespace A {
		function fn(a: string): void;
		function generic<T>(a: T): T;
		const a: string;
		type T = string;
		interface B {
			a: number;
		}
	}

	declare namespace Outer.Inner {
		const a: number;
	}
</script>
