<!-- An empty {:catch} is a rejection handler, so tsv keeps it (prettier deletes it). In the block (multiline) form the {:catch error} marker sits on its own line and {/await} on the next — no blank line between them, whether the block was authored across lines or reached that layout by breaking. -->

<!-- empty :catch after a multi-node pending body -->
{#await promise}
	<p>a</p>
	<p>b</p>
{/await}

<!-- full form (pending + then bodies), empty :catch -->
{#await promise}
	<p>a</p>
	<p>b</p>
{:then value}
	<p>c</p>
	<p>d</p>
{/await}
