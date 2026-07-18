<!-- An empty {:catch} is a rejection handler, so tsv keeps it (prettier deletes it). In the block form the kept-but-empty section keeps its own line, so {:catch error} and {/await} sit on their own lines with the empty body's blank line between them. -->

<!-- empty :catch after a multi-node pending body -->
{#await promise}
	<p>a</p>
	<p>b</p>
{:catch error}
{/await}

<!-- full form (pending + then bodies), empty :catch -->
{#await promise}
	<p>a</p>
	<p>b</p>
{:then value}
	<p>c</p>
	<p>d</p>
{:catch error}
{/await}
