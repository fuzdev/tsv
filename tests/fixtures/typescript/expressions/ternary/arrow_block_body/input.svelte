<script lang="ts">
	// Arrow block body in ternary consequent
	const a = cond
		? (e) => {
				fn(e);
			}
		: undefined;

	// In object property value
	const b = {
		prop: cond
			? (e) => {
					fn(e);
				}
			: undefined,
	};

	// Arrow block body in ternary alternate
	const c = cond
		? undefined
		: (e) => {
				fn(e);
			};

	// Multiple statements in block
	const d = cond
		? (a, b) => {
				log(a);
				log(b);
				return a + b;
			}
		: null;

	// Async arrow block body
	const e = cond
		? async (x) => {
				await fn(x);
			}
		: undefined;

	// Arrow with type annotation
	const f: Fn = cond
		? (x: string): void => {
				fn(x);
			}
		: undefined;

	// Nested ternary with arrows
	const g = a
		? (x) => {
				f1(x);
			}
		: b
			? (y) => {
					f2(y);
				}
			: null;
</script>
