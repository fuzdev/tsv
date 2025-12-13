<script lang="ts">
	class Base {
		method() {}
		get prop() {
			return 1;
		}
		static staticMethod() {}
	}

	class Derived extends Base {
		override method() {}
		override get prop() {
			return 2;
		}
	}

	class Multi extends Base {
		public override method() {}
		protected override other() {}
		static override staticMethod() {}
	}

	abstract class AbstractBase {
		abstract doThing(): void;
	}

	class Concrete extends AbstractBase {
		override doThing() {}
	}
</script>
