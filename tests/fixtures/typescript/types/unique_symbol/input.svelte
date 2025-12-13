<script lang="ts">
	declare const sym: unique symbol;
	const local: unique symbol = Symbol('local');

	interface A {
		[sym]: string;
	}

	type Key = typeof sym;
</script>
