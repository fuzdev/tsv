<script lang="ts">
	class A {
		// A typeless index signature (no `: ValueType`) is valid grammar; the missing
		// value type is a tsc checker error (TS1021), not a parse error.
		[a: string];
	}

	class B {
		// With a readonly modifier
		readonly [a: string];
	}

	class C {
		// With a static modifier
		static [a: string];
	}
</script>
