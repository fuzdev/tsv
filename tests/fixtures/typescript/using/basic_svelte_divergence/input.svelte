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
</script>
