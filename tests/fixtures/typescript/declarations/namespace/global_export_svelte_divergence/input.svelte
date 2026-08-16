<script lang="ts">
	// An exported global augmentation, in both its spellings
	export global {
		interface A {
			a: string;
		}
	}

	export declare global {
		interface B {
			b: string;
		}
	}
</script>
