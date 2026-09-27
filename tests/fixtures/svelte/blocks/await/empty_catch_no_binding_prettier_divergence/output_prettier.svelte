<!-- an empty {:catch} with no binding still handles a rejection, so it is kept -->

<!-- catch-shorthand, empty body -->
{#await promise}{/await}

<!-- empty {:catch} after a pending body -->
{#await promise}text{/await}

<!-- empty {:catch} after a then-shorthand body -->
{#await promise then value}{value}{/await}

<!-- block form: the kept section keeps its own line -->
{#await promise}
	<p>a</p>
	<p>b</p>
{/await}

<!-- block form after a newline-authored empty pending branch -->
{#await promise}{/await}

<!-- inside <pre> -->
<pre>{#await promise}text{/await}</pre>

<!-- inside <pre> a whitespace-only catch body is rendered text -->
<pre>{#await promise}text{/await}</pre>
