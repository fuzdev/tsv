<script lang="ts">
	class A {
		// Property decorator
		@d
		a: string;

		// Property decorator with value
		@d1
		b = 0;

		// Method decorator
		@d2
		fn() {}

		// Method decorator with args
		@d3('value')
		fn1() {
			return 42;
		}

		// Multiple decorators on accessor
		@d4
		@d5(false)
		get c(): number {
			return 0;
		}

		// Accessor keyword decorator
		@d6
		accessor e = 0;

		// Static decorated
		@d7
		static f = 'value';

		// Setter decorator
		@d8
		set g(v: number) {}
	}
</script>
