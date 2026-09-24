<!-- then shorthand, and the :then / :catch branches -->
{#await p then [a.e]}
	<p>{a.e}</p>
{/await}
{#await p}
	loading
{:then [...a.f]}
	<p>{a.f}</p>
{:catch { k: a.g }}
	<p>{a.g}</p>
{/await}
