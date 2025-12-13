<script lang="ts">
	const key = 'method';

	class Basic {
		[key]() {}
	}

	class WithSymbol {
		*[Symbol.iterator]() {
			yield 1;
		}
	}

	class WithGetterSetter {
		get [key]() {
			return 1;
		}
		set [key](v: number) {}
	}

	class WithModifiers {
		public [key]() {}
		private ['private']() {}
		#private() {}
		protected async ['async']() {}
		static ['static']() {}
	}
</script>
