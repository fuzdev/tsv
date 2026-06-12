<script lang="ts">
	// Simple identifier - fits inline, collapses from multiline
	const a = `value: ${

		expr
	  }`;

	// Property access - fits inline, collapses from multiline
	const b = `path: ${



		obj.prop


	 }`;

	// Short function call - fits inline, collapses from multiline
	const c = `result: ${

				fn(x)

	}`;

	// Array access - fits inline, collapses from multiline
	const d = `item: ${

	arr[0]

	}`;

	// Short ternary - fits inline, collapses from multiline
	const e = `flag: ${

	a ? b : c

	}`;

	// Template only contains interpolation
	const f = `${

					expr

	}`;

	// Logical AND - fits inline, collapses from multiline
	const g = `check: ${

		a && b

	}`;

	// Nullish coalescing - fits inline, collapses from multiline
	const h = `fallback: ${

		a ?? b

	}`;

	// TSAsExpression - fits inline, collapses from multiline
	const i = `typed: ${

		x as T

	}`;

	// TSSatisfiesExpression - fits inline, collapses from multiline
	const j = `typed: ${

		x satisfies T

	}`;

	// SequenceExpression - fits inline, collapses from multiline
	const k = `seq: ${

		(a, b)

	}`;

	// Await expression - fits inline, collapses from multiline
	const l = `fetched: ${

		await fn()

	}`;
</script>
