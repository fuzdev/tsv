<script lang="ts">
	class Basic {
		*gen() {
			yield 1;
		}
	}

	class AsyncGenerator {
		async *asyncGen() {
			yield 1;
		}
	}

	class Static {
		static *gen() {
			yield 1;
		}
		static async *asyncGen() {
			yield 1;
		}
	}

	class WithModifiers {
		public *gen() {
			yield 1;
		}
		private *privateGen() {
			yield 2;
		}
		*#privateGen() {
			yield 2;
		}
		protected *protectedGen() {
			yield 3;
		}
	}

	class WithTypes {
		*typed(): Generator<number> {
			yield 1;
		}
		async *asyncTyped(): AsyncGenerator<number> {
			yield 1;
		}
	}
</script>
