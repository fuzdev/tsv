<script lang="ts">
	// Global interface augmentation
	declare global {
		interface A {
			prop: string;
		}
	}

	// Multiple global augmentations
	declare global {
		interface Array<T> {
			customMethod(): T[];
		}

		interface String {
			customProp: number;
		}
	}

	// Global namespace
	declare global {
		namespace Outer {
			interface Inner {
				prop: string;
			}
		}
	}

	// Global variable
	declare global {
		var a: string;
		const b: number;
		function fn(): void;
	}

	// Empty global (module marker)
	declare global {}
</script>
