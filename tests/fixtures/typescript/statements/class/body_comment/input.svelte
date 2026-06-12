<script lang="ts">
	class A {
		// Leading comment on property
		a: string;

		/* Block comment */
		b: number;

		c: boolean; // Trailing comment

		// Leading comment on method
		fn() {}

		fn1() {} // Trailing on method
	}
</script>
