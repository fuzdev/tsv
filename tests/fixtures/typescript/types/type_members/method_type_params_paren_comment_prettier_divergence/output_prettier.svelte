<script lang="ts">
	// Interface method
	interface A {
		m<T>(/* c */ x: T): void;
	}

	// Type literal method
	type B = {
		m<T>(/* c */ x: T): void;
	};
</script>
