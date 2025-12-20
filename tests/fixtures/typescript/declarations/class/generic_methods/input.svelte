<script lang="ts">
	class A {
		// Single type parameter
		get<T>(a: string): T {
			return {} as T;
		}

		// Constrained type parameter
		lookup<K extends keyof T>(a: K): T[K] {
			return {} as T[K];
		}

		// Default type parameter
		decode<T = any>(a: unknown): T {
			return a as T;
		}

		// Async generic method
		async fetch<T>(a: string): Promise<T> {
			return {} as T;
		}

		// Multiple type parameters
		map<T, U>(a: T, fn: (x: T) => U): U {
			return fn(a);
		}

		// Static generic method
		static create<T>(a: T): A {
			return new A();
		}

		// Private generic method
		#private<T>(a: T): T {
			return a;
		}

		// Generator generic method
		*gen<T>(a: T): Generator<T> {
			yield a;
		}

		// Async generator generic method
		async *asyncGen<T>(a: T): AsyncGenerator<T> {
			yield a;
		}

		// Const type parameter (TS 5.0+)
		literal<const T>(a: T): T {
			return a;
		}
	}
</script>
