<script lang="ts">
	class A {
		// Type with initializer - full line 100 chars (no wrapping)
		aaaa: Map<T, { fn1: (x: any) => void; fn2: (x: any) => void; fn3: (x: any) => void }> =
			new Map();

		// Type with initializer - full line 101 chars (breaks at =)
		aaaaa: Map<T, { fn1: (x: any) => void; fn2: (x: any) => void; fn3: (x: any) => void }> =
			new Map();
	}
</script>
