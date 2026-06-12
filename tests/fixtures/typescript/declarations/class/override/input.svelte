<script lang="ts">
	// Base class
	class Base {
		a = 1;
		method() {}
		get prop() {
			return 1;
		}
		static staticMethod() {}
	}

	// Override on methods
	class MethodOverride extends Base {
		override method() {}
		override get prop() {
			return 2;
		}
		public override other() {}
		static override staticMethod() {}
	}

	// Override on fields
	class FieldOverride extends Base {
		override a = 2;
		public override b = 1;
		protected override c = 2;
		private override d = 3;
		override readonly e = 4;
		public override readonly f = 5;
	}

	// Override with type annotation
	class WithType extends Base {
		override a: number = 1;
	}

	// Override with optional/definite
	class WithModifier extends Base {
		override a?: number;
		override b!: number;
	}

	// Abstract override
	abstract class AbstractBase {
		abstract doThing(): void;
	}
	class Concrete extends AbstractBase {
		override doThing() {}
	}

	// Override as property name (not modifier)
	class OverrideAsName {
		override = 1;
	}
</script>
