<script lang="ts">
	class Basic {
		static x: number;
		static {
			this.x = 1;
		}
	}

	class Multiple {
		static a: number;
		static b: number;
		static {
			this.a = 1;
		}
		static {
			this.b = 2;
		}
	}

	class WithPrivate {
		static #secret = 'hidden';
		static {
			console.log(this.#secret);
		}
	}

	class Complex {
		static data: string[];
		static {
			try {
				this.data = ['a', 'b', 'c'];
			} catch {
				this.data = [];
			}
		}
	}
</script>
