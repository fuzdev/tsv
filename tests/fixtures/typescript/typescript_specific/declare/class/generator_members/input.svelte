<script lang="ts">
	declare class Single {
		*gen(): Iterator<number>;
		async *asyncGen(): AsyncIterator<number>;
		static *staticGen(): Iterator<void>;
		private *privateGen(): Iterator<void>;
	}
</script>
