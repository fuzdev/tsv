<script lang="ts">
	// Bare global augmentation (no `declare` keyword)
	global {
		interface A {
			a: string;
		}
	}

	// Global augmentation nested in an ambient module (declare is implicit)
	declare module 'foo' {
		global {
			interface B {
				b: string;
			}
		}
	}
</script>
