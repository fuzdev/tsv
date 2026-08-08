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
		abstract override a: number;
		abstract override method(): number;
		abstract override get prop(): number;
		abstract override set other(v: string);
	}

	// An accessibility modifier precedes the pair
	abstract class B extends Base {
		public abstract override a: number;
		protected abstract override method(): number;
	}

	// A method named `get` with type parameters, still `abstract override`
	class C {
		get<T extends string>(): T {
			return 'a' as T;
		}
	}
	abstract class D extends C {
		abstract override get<T extends string>(): T;
	}
</script>
