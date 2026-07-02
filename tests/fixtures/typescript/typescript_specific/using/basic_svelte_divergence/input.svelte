<script lang="ts">
	// Basic using declaration
	using resource = getResource();

	// Using with type annotation
	using typed: Disposable = getResource();

	// Multiple declarators
	using a = x,
		b = y;

	// Using with null (optional resource)
	using nothing = null;

	// Using in block scope
	{
		using scoped = getResource();
	}

	// Using in function
	function process() {
		using resource = acquire();
		return resource.getValue();
	}

	// Using in for-of loop (disposed each iteration)
	function iterate(resources: Iterable<Disposable>) {
		for (using resource of resources) {
			console.log(resource.getValue());
		}
	}

	// Contextual keywords as binding names — any identifier-shaped word that
	// is not an expression continuation (`in`/`instanceof`/`as`/`satisfies`)
	// binds, even when it lexes as a keyword
	using async = getResource();
	using undefined = getResource();
	using of = getResource();

	// Contextual-keyword binding in a for-of head (only `of` itself is
	// excluded there)
	function keywords(resources: Iterable<Disposable>) {
		for (using async of resources) {
			console.log(async.getValue());
		}
	}
</script>
