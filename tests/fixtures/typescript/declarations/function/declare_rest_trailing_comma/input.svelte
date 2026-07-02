<script lang="ts">
	declare function fn1(...a: T[]): void;

	declare function fn2(cb: (...a: T[]) => void): void;

	declare class A {
		m(...a: T[]): R;
	}

	declare interface I {
		m(...a: T[]): R;
	}
</script>
