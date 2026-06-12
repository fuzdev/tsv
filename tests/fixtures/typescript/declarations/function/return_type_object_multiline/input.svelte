<script lang="ts">
	// Params should stay inline when they fit, even with multiline return type
	function fn1(a: number): {
		x: number;
		y: number;
	} {}

	export function fn2<T>(a?: () => T): {
		b: (c?: string) => T;
		d: () => T | undefined;
		e: (f?: T) => T;
	} {}
</script>
