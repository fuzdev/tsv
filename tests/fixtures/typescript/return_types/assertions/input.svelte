<script lang="ts">
	// asserts with type predicate (function)
	function assertStr(x: unknown): asserts x is string {
		if (typeof x !== 'string') throw new Error('err');
	}

	function assertNum(x: unknown): asserts x is number {
		if (typeof x !== 'number') throw new Error('err');
	}

	// asserts without type predicate (just truthiness)
	function assertVal(x: unknown): asserts x {
		if (!x) throw new Error('err');
	}

	function assertDef<T>(x: T | undefined): asserts x is T {
		if (x === undefined) throw new Error('err');
	}

	// asserts this is T
	class Container<T> {
		value: T | undefined;

		assertHasValue(): asserts this is {value: T} {
			if (this.value === undefined) throw new Error('err');
		}
	}

	// asserts this (without is T)
	class Truthy {
		assertThis(): asserts this {
			if (!this) throw new Error('err');
		}
	}

	// Arrow with asserts (type annotation on variable)
	const assertTruthy: (x: unknown) => asserts x = (x) => {
		if (!x) throw new Error('err');
	};

	// Arrow with asserts (inline return type)
	const assertDefined2 = (x: unknown): asserts x => {
		if (x === undefined) throw new Error('err');
	};

	// Method with asserts
	class Checker {
		assertObj(x: unknown): asserts x is object {
			if (typeof x !== 'object') throw new Error('err');
		}
	}

	// Declare function variants (already working, include for completeness)
	declare function assertExists<T>(x: T | null | undefined): asserts x is T;
	declare function assertNonNull<T>(x: T): asserts x is NonNullable<T>;
	declare function assertTrue(x: unknown): asserts x;
</script>
