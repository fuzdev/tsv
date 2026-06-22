<script lang="ts">
	class A {
		// Type with initializer - full line 100 chars (no wrapping)
		aa: Map<T, { fn1: (x: any) => void; fn2: (x: any) => void; fn3: (x: any) => void }> = new Map();

		// Type with initializer - full line 101 chars (breaks at =)
		aaa: Map<T, { fn1: (x: any) => void; fn2: (x: any) => void; fn3: (x: any) => void }> =
			new Map();
	}
</script>
