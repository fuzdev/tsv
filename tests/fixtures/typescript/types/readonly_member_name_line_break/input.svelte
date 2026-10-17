<script lang="ts">
	interface Flags {
		readonly;
		enabled: boolean;
	}

	interface Lookup {
		readonly;
		[key: string]: unknown;
	}

	interface Factory {
		readonly;
		new (): Factory;
	}

	type Handle = {
		readonly;
		close(): void;
	};
</script>
