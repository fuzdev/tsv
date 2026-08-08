<script lang="ts">
	class Base {
		a: number = 1;
		method(): number {
			return 1;
		}
		get prop(): number {
			return 1;
		}
		set other(v: string) {}
	}

	// `abstract override` is the canonical modifier order (tsc's own spelling)
	abstract class A extends Base {
		override abstract a: number;
		override abstract method(): number;
		override abstract get prop(): number;
		override abstract set other(v: string);
	}

	// An accessibility modifier precedes the pair
	abstract class B extends Base {
		public override abstract a: number;
		protected override abstract method(): number;
	}

	// A method named `get` with type parameters, still `abstract override`
	class C {
		get<T extends string>(): T {
			return 'a' as T;
		}
	}
	abstract class D extends C {
		override abstract get<T extends string>(): T;
	}
</script>
