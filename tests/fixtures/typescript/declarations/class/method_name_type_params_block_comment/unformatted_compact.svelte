<script lang="ts">
	class A {
		fn1/* c1 */<T>() {}
	}

	const b = {
		fn1/* c2 */<T>() {},
	};

	interface C {
		fn1/* c3 */<T>(): void;
	}

	type D = {
		fn1/* c4 */<T>(): void;
	};
</script>
