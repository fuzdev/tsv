<script lang="ts">
	//
	// Generic Function Declarations
	//

	// Single type parameter
	function single<T>(x: T): T {
		return x;
	}

	// Multiple type parameters
	function multiple<T, U>(a: T, b: U): [U, T] {
		return [b, a];
	}

	// With constraint
	function withConstraint<T extends {length: number}>(a: T, b: T): T {
		return a.length >= b.length ? a : b;
	}

	// With default
	function withDefault<T = string>(): T | undefined {
		return undefined;
	}

	// Constraint and default
	function withBoth<T extends object = {}>(obj: T): T {
		return obj;
	}

	// Complex: keyof constraint
	function complex<T, K extends keyof T>(obj: T, keys: K[]): Pick<T, K> {
		const result = {} as Pick<T, K>;
		return result;
	}

	// Async generic
	async function asyncGeneric<T>(url: string): Promise<T> {
		const response = await fetch(url);
		return response.json();
	}

	// Generator generic
	function* generatorGeneric<T>(items: T[]): Generator<T> {
		for (const item of items) {
			yield item;
		}
	}

	// Async generator generic
	async function* asyncGeneratorGeneric<T>(items: T[]): AsyncGenerator<T> {
		for (const item of items) {
			yield item;
		}
	}

	// Exported generic
	export function exported<T>(x: T): T {
		return x;
	}

	// Exported async generic
	export async function exportedAsync<T>(x: T): Promise<T> {
		return x;
	}
</script>
